//! The app's self-check suite. Checks the current tree: computed values, source scans, and the real binary
//! started as a child process.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Assert that an action answered as `pat`; failing, the answer itself is printed (after the given words, if
/// any), so a run on another machine says what came back instead of only that it did not match.
macro_rules! answers {
    ($got:expr, $pat:pat) => {{
        let got = $got;
        assert!(matches!(got, $pat), "expected {}, answered {:?}", stringify!($pat), got);
    }};
    ($got:expr, $pat:pat, $($said:tt)+) => {{
        let got = $got;
        assert!(matches!(got, $pat), "{}: expected {}, answered {:?}", format!($($said)+), stringify!($pat), got);
    }};
}

/// Start a child and collect its output like `Command::output` (stdin closed, both streams captured), via
/// `zikaron_os::spawn`, where the child closes every listed kernel lock before it runs, so a lock a test releases
/// is never held by a child started on another thread (`zikaron_os::LockFile`). All children go through here.
fn output_of(cmd: &mut Command) -> std::io::Result<std::process::Output> {
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    zikaron_os::spawn(cmd)?.wait_with_output()
}

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// One app source file by module file name. A folder module (`x/mod.rs` and its children) reads as one text:
/// `mod.rs` first, then the others in name order.
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

/// The shipped sources: each lib module (a folder module as one text named `<folder>.rs`) and the window
/// binary.
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

/// Open the vault these tests seal local data under (sealing needs the vault's master key): set the places
/// once for this process (a temporary machine directory, the app's own account name; the real machine
/// directory is never touched), set a passcode and open the vault.
fn vault_open() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let machine = checks_machine();
        let _ = std::fs::remove_dir_all(&machine);
        // The directory is removed when the test process exits, so no run leaves it behind.
        extern "C" fn tidy() {
            let _ = std::fs::remove_dir_all(checks_machine());
        }
        unsafe extern "C" {
            fn atexit(f: extern "C" fn()) -> i32;
        }
        unsafe {
            atexit(tidy);
        }
        app::places::set(app::places::Places { key_account: app::places::ACCOUNT.to_string(), machine_dir: Some(machine), user_home: None });
        app::keybox::set_pin("27618394").expect("开得了这一趟的库");
    });
}

/// This process's machine directory: under the temp directory, or `/tmp` when that path is long (the
/// desktop's command-line door is a local socket there, and socket paths have a small limit).
fn checks_machine() -> PathBuf {
    let t = std::env::temp_dir();
    let base = if t.as_os_str().len() <= 40 { t } else { PathBuf::from("/tmp") };
    base.join(format!("zk-checks-machine-{}", std::process::id()))
}

fn code_only(s: &str) -> String {
    s.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

// ───────────────────── Design values live in the widget library ─────────────────────

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
    let out = output_of(Command::new(env!("CARGO_BIN_EXE_app"))
        .arg("palette")
        )
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
    // `join` may appear only in shutdown: joining a handle in the frame would block.
    let shutdown = code.split("pub fn shutdown").nth(1).expect("没有 shutdown");
    let joins_total = code.matches(".join()").count();
    let joins_in_shutdown = shutdown.matches(".join()").count();
    assert_eq!(joins_total, joins_in_shutdown, "join 只许住 shutdown");
    assert_eq!(joins_in_shutdown, 1, "关门恰一处 join");
}

#[test]
fn exactly_one_place_starts_a_thread() {
    // Threads start only via `task::start_thread` (a `thread::Builder`, so a refused thread is an error, not a
    // panic); the panicking `thread::spawn` appears nowhere.
    let (mut built, mut bare) = (0, 0);
    for (_, text) in shipped() {
        built += code_only(&text).matches("thread::Builder").count();
        bare += code_only(&text).matches("thread::spawn").count();
    }
    assert_eq!((built, bare), (1, 0), "起线程只许一处(它在单飞那一问之后),且不用会崩的那一口");
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

/// Every legal action names its equivalent CLI verb. Exactly the nine legal actions are passed in, each first
/// asserted legal, so the assertion cannot pass vacuously.
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
    // Conversely, actions that create no legal fact (landing place, local preferences) name no verb.
    for a in [Action::SelfCheck, Action::Quit] {
        assert!(!a.is_legal(), "{a:?} 不该是法律动作");
    }
}

// ───────────────────── No disk in the frame ─────────────────────


/// The frame never reads the disk: directory walks and ledger queries happen only in the background pass
/// (`Action::Measure`), never in the window module.
#[test]
fn the_frame_never_touches_the_disk() {
    let text = read_src_file("window.rs").expect("读不出 window.rs");
    let code = code_only(&text);
    for needle in ["usage()", ".survey()", "read_dir", "std::fs::", "metadata("] {
        assert!(!code.contains(needle), "window.rs 里有 {needle}:读盘要走后台那一趟");
    }
    // The background pass itself exists: the home-measuring kind is in the closed table.
    assert!(app::task::Kind::ALL.contains(&app::task::Kind::Archive));
}

// ═════════════════════ Identity and keys ═════════════════════

/// No system keychain interface anywhere: keys live in the local key vault (`keybox`). Scans every
/// package's `src/` under `crates/` for keychain symbols.
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
                // Scan only each package's source (`src/`).
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
    // UI and refusal text never mention the system keychain, or a user might search it for a key that never
    // lived there. Fault code names (such as `KEYCHAIN_MISSING`) are stable identifiers and are exempt.
    let hits = |t: &str| t.contains("钥匙串") || t.to_lowercase().contains("keychain");
    for (k, zh, en) in app::lang::TABLE.iter() {
        assert!(!hits(zh) && !hits(en), "话册 {k:?} 的人话还说钥匙串");
    }
    for k in app::fault::Known::ALL {
        assert!(!hits(app::fault::translate(k)), "拒因 {} 的人话还说钥匙串", k.as_str());
    }
    // The file dialog has one definition; FFI lives only in the platform interface.
    let platform = code_only(&read_src_file("platform.rs").expect("平台接口那一处"));
    assert_eq!(platform.matches("pub fn ask_path(").count(), 1, "选档框只此一处");
    for (name, text) in shipped() {
        if name == "platform.rs" {
            continue;
        }
        assert!(!code_only(&text).contains("extern \"C\""), "{name} 里有 FFI");
    }
}

/// OS capabilities go only through `platform/`: no platform crate, `HOME`, `/etc/localtime` or font directory
/// elsewhere, and no-window exits are reported through it rather than written to stderr.
#[test]
fn platform_capabilities_go_through_one_interface() {
    let platform = code_only(&read_src_file("platform.rs").expect("平台接口"));
    for f in ["pub fn ask_path(", "pub fn lock_now(", "pub fn lock_wait(", "pub fn home_dir(", "pub fn zone_rules(", "pub fn user_temp_dir(", "pub fn app_data_dir(", "pub fn say_without_window("] {
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
    // Three font tables: macOS and Windows (Chinese from the system) and all other systems (all embedded); each
    // system is named explicitly, never inferred as "not Linux".
    assert_eq!(fonts.matches("pub const ROLES:").count(), 3, "字体表三份(macOS、Windows 各一份,其余系统一份)");
    assert!(fonts.contains("#[cfg(target_os = \"macos\")]\npub const ROLES:") && fonts.contains("#[cfg(target_os = \"windows\")]\npub const ROLES:"), "macOS 与 Windows 两份各点名");
    assert!(!fonts.contains("not(target_os = \"linux\")"), "不以「不是 Linux」当 macOS");
}

/// The signing API exposes only two domains (law §5.7), one function each with the domain fixed in its body;
/// the kit law's fpm and ack domains are signed by the command line only.
#[test]
fn the_anchor_key_signs_two_faces_and_no_more() {
    use app::sign::{domains, Face};
    // Two faces, two domains, one to one.
    assert_eq!(Face::ALL.len(), 2, "两张脸,不多不少");
    let text = read_src_file("sign.rs").expect("读不出 sign.rs");
    let code = code_only(&text);
    // One function per face, its domain fixed in its body; none takes a domain parameter.
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
    // The domain parameter's type is closed, not `&str`.
    assert!(
        code.contains("fn seal(s: &Secret, preimage: &[u8], domain: Law)"),
        "seal 的第三格要收闭型"
    );
    assert!(
        !code.contains("domain: &str"),
        "sign.rs 里还有一处收自由域字符串的口"
    );
    // No domain literal is spelled in this file.
    for spelled in ["zikaron/1", "zikaron.fpm", "zikaron.ack", "personal_sign", "0x19", "7702"] {
        assert!(!code.contains(spelled), "sign.rs 里自拼了域字面 {spelled}");
    }
    // The recognized domains are exactly the two members of the core's closed type, one per face.
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
    // Names outside the faces are not recognized.
    assert!(Face::parse("zikaron/1").is_none(), "域的字面不是一张脸");
    assert!(Face::parse("kit").is_none() && Face::parse("ack").is_none(), "撤下的两张脸认不出");
    for f in Face::ALL {
        assert_eq!(Face::parse(f.as_str()), Some(f), "{} 认不回自己", f.as_str());
    }
}

/// The settings file holds no key material, and BIP-39 words are only generated and handled by the identity
/// layer (the word list crate and the entropy-to-words exit live in one or two places).
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
    // The word list crate lives only in cryptx; entropy-to-words is called only by the identity layer and
    // cryptx; plaintext words (`Fresh::words`) are taken only by the identity layer and the window (for display).
    // Neither writes them to disk, so plaintext words have no path to our files.
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

/// Revealing a raw private key is one-time: the function takes the `Secret`, leaving nothing the second time.
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

/// No path writes a plaintext private key to disk: no file-writing code takes a `Secret`'s bytes.
#[test]
fn no_path_writes_a_plaintext_key_to_disk() {
    let text = read_src_file("key.rs").expect("读不出 key.rs");
    let code = code_only(&text);
    for w in ["fs::write", "File::create", "land_bytes"] {
        assert!(!code.contains(w), "key.rs 里有落盘的路 {w}:明文只许住内存与系统钥匙串");
    }
    // The plaintext accessor is private to its module.
    assert!(code.contains("\n    fn bytes(&self)"), "bytes() 要住 key.rs 自己一处");
    assert!(!code.contains("pub(crate) fn bytes"), "bytes() 不许对 crate 公开");
    assert!(!code.contains("pub fn bytes"), "bytes() 不许对外公开");
    // The exits are exactly these, and none hands out plaintext that could be written to disk.
    for exit in [
        // The key-lending exit is visible only to `sign`, so no other file can even write a call that signs
        // arbitrary domain strings.
        "pub(in crate::sign) fn with_sign_key<R>(&self, f: impl FnOnce(&[u8; 32]) -> R)",
        "pub(crate) fn ciphered(&self, key: &[u8; 16], iv: &[u8; 16])",
        // Likewise the anchoring exit: visible only to `sign`, and the lending closure runs the send itself.
        "pub(in crate::sign) fn with_tx_key<R>(&self, f: impl FnOnce(&[u8; 32]) -> R)",
        "pub fn reveal_once(slot: &mut Option<Secret>)",
    ] {
        assert!(code.contains(exit), "少了这一处出口:{exit}");
    }
    // The only path to disk is through `cryptx::aes128_ctr`: the keystore receives ciphertext.
    let ks = code_only(&read_src_file("keystore.rs").expect("读不出 keystore.rs"));
    assert!(ks.contains("secret.ciphered("), "keystore 要走密文那一口");
    assert!(!ks.contains(".bytes()"), "keystore 里不许再摸明文字节");
    // Each lending exit has exactly one call site, next to its owner.
    for (口, 住处, 邻居) in [
        // The key is lent only to the signing step (`send::sign_at`, with a nonce fetched beforehand); submitting
        // the signed bytes to each endpoint borrows no key.
        ("with_tx_key(", "sign.rs", "send::sign_at("),
        ("with_sign_key(", "sign.rs", "cryptox::sign_digest("),
    ] {
        let mut sites = 0usize;
        let mut beside = false;
        for name in [
            "action.rs", "window.rs", "keystore.rs", "entryx.rs", "sign.rs", "mirror.rs",
            "firstrun.rs", "key.rs", "ledgerx.rs", "anchorx.rs", "queue.rs", "auditx.rs",
        ] {
            let t = code_only(&read_src_file(name).expect("读不出源"));
            // The two in `key.rs` are definitions, not calls: subtract them.
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

/// The home lays out its own four subdirectories (`lay`); nothing is set up by hand.
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

/// Locks come from the kernel and no code deletes lock files, so a killed process leaves no stale lock.
#[test]
fn the_lock_is_the_kernels_and_nothing_deletes_it() {
    let text = read_src_file("lock.rs").expect("读不出 lock.rs");
    let code = code_only(&text);
    assert!(code.contains("crate::platform::lock_now("), "锁要向内核要(经平台接口)");
    assert!(code_only(&read_src_file("platform.rs").expect("平台接口")).contains("fn flock("), "平台那一处向内核要 flock");
    for w in ["remove_file", "unlink", "stale", "陈旧", "pid ==", "SystemTime"] {
        assert!(!code.contains(w), "lock.rs 里有 {w}:陈旧锁的判词一旦出现,就是那条渐近线");
    }
    // In the shipped build, only lock.rs touches the lock file.
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

/// Any copy of a home is equivalent: nothing inside the home records the home's own path (the mirror record
/// holds a user-chosen location, which is checked not to be this home).
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
    // Fill the three tables too: endpoints, remembered addresses and exclusive flags all live in tables.
    s.endpoints = vec!["31337=http://127.0.0.1:8545".to_string()];
    s.book = vec!["0x1111111111111111111111111111111111111111".to_string()];
    s.exclusive = vec!["0x22".to_string()];
    s.write(&h).expect("写设置");
    // Settings are sealed local data: read the document through the one sealing entry point.
    let at = h.dir(app::home::Slot::Settings).join(app::settings::FILE);
    let bytes = app::local::read(&at, app::local::Doc::Settings).expect("读设置").expect("设置在");
    let text = String::from_utf8_lossy(&bytes).to_string();
    assert!(
        !text.contains(&dir.display().to_string()),
        "设置档里写了它自己在哪:{text}"
    );
    // Check every string cell for this home's path, walking the whole document including arrays: exempting a
    // field named `path` would also exempt `repo`, and walking only objects would miss the `endpoints`, `book`
    // and `exclusive` tables.
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

/// Walk every string cell in a JSON value, entering both objects and arrays (skipping arrays would leave
/// table strings unseen and assertions built on the walk empty).
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

/// No first-run checklist point asks about a directory the app never creates (it would stay red forever).
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
    // Directory names joined onto paths are among those the app creates.
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
    // Scan only path-building files: elsewhere `join` joins strings (`Vec::join`), not paths.
    for name in ["home.rs", "mirror.rs", "firstrun.rs", "settings.rs", "lock.rs", "keystore.rs"] {
        let text = read_src_file(name).expect("读不出源");
        for piece in code_only(&text).split(".join(\"").skip(1) {
            let lit = piece.split('"').next().unwrap_or("");
            // A path name has at least one letter or digit; whitespace or punctuation literals are string separators
            // for `Vec::join`, not paths.
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

/// Every legal action names its equivalent CLI verb, and that verb is in the CLI-SCHEMA.md verb table.
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

/// An empty locked-out vault can be reset: the gate's "reset key vault" button and `reset_empty` both ask
/// `keybox::recoverable`, so the button never shows only to be refused.
#[test]
fn the_way_out_of_an_empty_locked_box_and_the_key_that_offers_it_ask_one_question() {
    let keybox = read_src_file("keybox.rs").expect("读不出 keybox.rs");
    let kcode = code_only(&keybox);
    // The refusal asks the question itself instead of recounting slots and seals.
    assert!(
        kcode.contains("pub fn reset_empty()") && kcode.contains("if recoverable()?"),
        "`reset_empty` 要问 `recoverable()`,不许自己再拼一遍条件"
    );
    // It deletes the whole file, and always asks first.
    let body = kcode.split("pub fn reset_empty()").nth(1).unwrap_or("");
    let asked = body.find("recoverable()");
    let removed = body.find("remove_file");
    assert!(asked.is_some() && removed.is_some() && asked < removed, "`reset_empty` 要先问再删");
    // The window button's condition reads the same answer (the shell field is set from `recoverable` when the
    // vault is reread).
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
    // The action needs no key: with the vault locked every key-needing action is refused, and this one must pass.
    assert!(!app::action::Action::ResetEmptyKeybox.needs_key(), "重置空库那一条不该要钥");
}

/// Whether the gate covers the window is one closed table over the vault states (`State::gate_up`), so a
/// brand-new machine with no vault still gets the wizard.
#[test]
fn whether_the_gate_covers_the_window_is_one_closed_table_over_the_four_vault_states() {
    use app::keybox::State;
    let all = [State::Absent, State::Locked { wrong: 0 }, State::Locked { wrong: 4 }, State::LockedOut, State::Open];
    for s in all {
        // Both questions are closed; check every member.
        let gate = s.gate_up();
        let keys = s.keys_ready();
        let want_gate = matches!(s, State::Locked { .. } | State::LockedOut);
        let want_keys = matches!(s, State::Open);
        assert_eq!(gate, want_gate, "{s:?}:门在不在答错了");
        assert_eq!(keys, want_keys, "{s:?}:钥拿不拿得到答错了");
        // With no vault the shell still draws, so the wizard can stand and the user can set a passcode.
        if matches!(s, State::Absent) {
            assert!(!gate, "还没有库时门不该摆在屏上(否则首启走不出去)");
            assert!(!keys, "还没有库时钥也拿不到");
        }
    }
    // Every place in the window asking "is the gate up" uses this table. Six places: whether the shell draws,
    // the wizard shows, cards show, the gate draws, "key vault locked" toasts, and the backup sheets float above
    // the gate (restoring from the locked card).
    let text = read_src_file("window.rs").expect("读不出 window.rs");
    let code = code_only(&text);
    assert_eq!(
        code.matches("vault.gate_up()").count() + code.matches("state.gate_up()").count(),
        6,
        "问「门在不在」的该是六处,都过 `State::gate_up`(现读到的不是六处)"
    );
    // No other spelling of the question: neither "is it open" as the draw condition nor `matches!` on the states.
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
    // The shell's key availability matches `State::keys_ready` in every state (compared by behaviour).
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

/// The shipped build has no way to change where the anchor key lives: no environment variable can choose
/// it (one could quietly swap the signing key), and setting places is for test hooks only.
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
        // Likewise the light KDF level (`n=2`) is for tests only: shipped, it would make an offline brute force of
        // an eight-digit passcode take seconds. Only its definition may appear, in `keybox.rs`.
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
    // When never set, the app's own places apply.
    assert_eq!(app::places::key_account(), app::places::ACCOUNT);
    // Vaults record their KDF level as written by `params()`, which the shipped build never lightens, so
    // shipped vaults are always standard; the level read back is bounds-checked, so a tampered file cannot
    // demand an absurd amount of memory.
    let keybox = read_src_file("keybox.rs").expect("读不出 keybox.rs");
    let code = code_only(&keybox);
    assert!(code.contains("book.kdf = Some(params())"), "建库那一趟该把这一趟的取式档写进库档");
    assert!(code.contains("crate::keystore::in_range(n, r, pp)"), "库档读回来那一节要现验界");
    assert!(code.contains("fn kdf_of(b: &Book)"), "开档那一侧该有「照这一本库那一档」的一处正主");
    assert_eq!(app::keybox::params(), app::keystore::Params::standard(), "没摆过轻档时跑的就是标准档");
}

// ═════════════════════ Bilingual base ═════════════════════

/// Both languages carry the same key set: each key has exactly one row, neither language is empty, and no two
/// keys share words in one language.
#[test]
fn the_two_languages_carry_the_same_key_set() {
    let bad = app::lang::trouble();
    assert!(bad.is_empty(), "键表不齐:{bad:?}");
    assert_eq!(app::lang::TABLE.len(), app::lang::Key::ALL.len());
    assert_eq!(app::lang::Lang::ALL.len(), 2);
    // Both languages answer, and switching language takes effect.
    for l in app::lang::Lang::ALL {
        app::lang::set(l);
        assert_eq!(app::lang::lang(), l);
        for k in app::lang::Key::ALL {
            assert!(!app::lang::t(k).is_empty(), "{k:?} 在 {} 下是空的", l.as_str());
        }
    }
    app::lang::set(app::lang::Lang::Zh);
}

/// The window has no hard-coded sentences: any CJK character in a window string literal bypasses the key table.
#[test]
fn the_window_carries_no_sentence_of_its_own() {
    let text = read_src_file("window.rs").expect("读不出 window.rs");
    for (i, l) in text.lines().enumerate() {
        if l.trim_start().starts_with("//") {
            continue;
        }
        // Check only characters inside string literals (comment lines are skipped above).
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

/// No sentence reaches the screen with an open slot: each `fillN` fills a sentence whose `{k}` slots are all
/// below N in both languages, and a sentence taken whole with `t` has no slots.
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

/// There is exactly one way to fetch a sentence.
#[test]
fn there_is_one_way_to_fetch_a_sentence() {
    let text = read_src_file("lang.rs").expect("读不出 lang.rs");
    let code = code_only(&text);
    assert_eq!(code.matches("pub fn t(").count(), 1, "取词的路恰一条");
    assert!(code.contains("pub const TABLE"), "一行同时带两种话的那张表该在这里");
}

// ═════════════════════ Whole-machine backup and restore ═════════════════════

/// A restore is all or nothing: stage the vault, stage machine files, rename the vault, then settle; a cut
/// before the rename leaves the machine as it was.
#[test]
fn a_restore_stages_everything_before_it_lands_anything() {
    let text = read_src_file("backup.rs").expect("读不出 backup.rs");
    // Cut at the next top-level `pub fn`, so these assertions cover `restore` only.
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

/// The pen is released only when the core audit's label is COMPLETE.
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

/// The single-source flag comes from the endpoint rule, not counted in this layer.
#[test]
fn the_single_source_column_comes_from_the_endpoint_law() {
    let text = read_src_file("chainx.rs").expect("读不出 chainx.rs");
    let code = code_only(&text);
    assert!(code.contains("single_source: r.single_source"), "单源那一栏要照抄端点法给的");
    assert!(!code.contains("sources == 1"), "别在这一层自己判单源");
}

// ═════════════════════ First run and adoption ═════════════════════

/// The shipped app starts no child processes at all, so none can be orphaned; background work runs on
/// threads joined in `shutdown`.
#[test]
fn the_shipped_app_starts_no_child_process_at_all() {
    for (name, text) in shipped() {
        let code = code_only(&text);
        for w in ["Command::new", "std::process::Command"] {
            assert!(!code.contains(w), "{name} 里起了子进程:发布件一个也不许起");
        }
    }
}

/// Adoption never moves or rewrites the foreign directory: each verified entry lands as a sealed copy in this
/// home's ledger (a hard link would put plain data inside the home).
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

// ═════════════════════ Ledger-law vocabulary comes from the core ═════════════════════

/// The two ledger-law words used here come from the core's closed types (the test keeps no roster of its
/// own, which would drift from the core's `tokens.rs`).
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

/// Opening a home is judged by what is on disk: the rooms must exist when asked again, not merely "no error".
#[test]
fn opening_a_home_is_judged_by_what_is_on_the_disk() {
    // A writer's lock writes this machine's mark into the test's own machine directory, never the real one.
    vault_open();
    // Behaviour, not source text: check whether the rooms exist on disk.
    let base = std::env::temp_dir().join(format!("zk-lay-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);

    // 1 · Laid out: the four rooms exist on disk (asked of the file system, not inferred from no error).
    let good = base.join("good");
    let h = app::home::Home::open_or_create(&good).expect("铺得出来");
    for s in app::home::Slot::ALL {
        assert!(h.dir(s).is_dir(), "{} 没落在盘上", s.as_str());
    }
    assert!(h.missing().is_empty(), "缺:{:?}", h.missing());

    // 2 · A room's name is taken by a file: refused by name ("cannot create"), the room named in the tail.
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

    // 3 · Opening the home through the action layer gives the same refusal.
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

    // 4 · "Cannot create directory", "no home yet" and "a writer already exists" are distinct sentences.
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

/// Every legal action names a verb from the `CLI-SCHEMA.md` §6 verb table, read now. The sample covers every
/// `Action` constructor.
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
    // The sample size is not pinned: it is at most the closed type's member count (`Action::NAMES`).
    assert!(!every.is_empty() && every.len() <= app::action::Action::NAMES.len(), "样本表比 `Action` 的闭表还多");
    // The verb table is read from `CLI-SCHEMA.md`, not copied here.
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
    // These actions each name their verb.
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

/// Every page has a name in both languages and an icon.
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

/// The inflater reads a real zlib stream (dynamic Huffman tables and back references): the sha256 of the
/// inflated bytes matches.
#[test]
fn the_inflater_reads_a_real_zlib_stream() {
    let z = zikaron::hexfmt::decode(&format!("0x{}", ZLIB_SAMPLE)).expect("样本是十六进制");
    let out = app::zlibx::inflate_zlib(&z, 1 << 20).expect("解得开");
    assert_eq!(out.len(), 970, "解出来的长度");
    assert_eq!(
        zikaron::hexfmt::encode(&zikaron::cryptox::sha256(&out)),
        "0x35ccdc6558f3343f59c1268be8e0fae8ce118b07cea54ceb2f75823b68accfb2"
    );
    // Truncated streams and bad headers fail at once; nothing is guessed.
    assert!(app::zlibx::inflate_zlib(&z[..z.len() / 2], 1 << 20).is_none(), "截断的流该停");
    assert!(app::zlibx::inflate_zlib(&[0x00, 0x00, 0x00], 1 << 20).is_none(), "坏头该停");
    // The output cap works: exceeding it stops (a compression bomb goes nowhere).
    assert!(app::zlibx::inflate_zlib(&z, 16).is_none(), "上限该拦住");
}

const ZLIB_SAMPLE: &str = "78daabcacc4e2ccacfd33754a81a650d02d652df5312aa06bb65738575ee5d53aed6bb29675f247f5ab250dc65ca354f9bb9312609fb0c15e432ffdd5af0eec5ce99f531359a33ffae7f3a59d5e65ac87adf5fd745d417acdbfcefa5b27ed72745f9794f261eddc8fd666b98f51fb9fcc94e75a74ffcd30c7d7ab6cfed4edf95ed87ca7cb5a27ccbcad97ec4b64d60f2bab677b183f4cb13a7cf9c34fd76563e5129eb61b0c53a2913065fe35dbc2a59077c1a37eefa6467fdf3ddd7eff3b53d4dd6b77f0de2cedce9cd3b43af7577e8b6a21545c955674bd2fe6ce3e3ebffd892bce1c9a65d9a0526251f52d6647c67f8ba41dbf658da97e87bab744ebd3dab1d18eec8e7fbceebd3667f672e7613f77bc9397c0d3953ab97b55c7396dffaeaba932fe7c358269f088f4fb6f2cbbe9bc9d627f68a1a3de75378b42cadf7f997ba96f4a76e57ed4e3c5aa85a7d3b3bb1a0243f2f335161940164000050688899";

/// A git commit's content hash, id, ancestor count and subject match what `git` computes, for both packed
/// and loose objects.
#[test]
fn the_git_content_hash_is_what_git_itself_says() {
    let repo = std::env::temp_dir().join(format!("zk-git-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&repo);
    std::fs::create_dir_all(&repo).expect("建目录");
    // An empty global config of the test's own, so the user's git config plays no part.
    let empty_config = repo.with_extension("gitconfig");
    std::fs::write(&empty_config, b"").expect("空设置档");
    let git = |args: &[&str]| {
        let o = output_of(Command::new("git")
            .args(args)
            .current_dir(&repo)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", &empty_config)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "")
            )
            .expect("起不动 git");
        assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    };
    git(&["init", "-q", "."]);
    for (i, body) in ["one", "two", "three"].iter().enumerate() {
        std::fs::write(repo.join("f.txt"), body.repeat(200)).expect("写");
        git(&["add", "f.txt"]);
        git(&["commit", "-q", "-m", &format!("commit {i}"), "-m", "second paragraph"]);
    }
    // Pack the history so far (with deltas between the versions of f.txt).
    git(&["repack", "-a", "-d", "-q"]);
    for i in 3..5 {
        std::fs::write(repo.join(format!("g{i}.txt")), format!("loose {i}")).expect("写");
        git(&["add", "."]);
        git(&["commit", "-q", "-m", &format!("commit {i}")]);
    }
    let got = app::gitx::head_of(&repo).expect("读得出 HEAD");

    let bytes = output_of(Command::new("git")
        .args(["cat-file", "commit", "HEAD"])
        .current_dir(&repo)
        )
        .expect("起不动 git");
    assert!(bytes.status.success(), "git cat-file 没成");
    assert_eq!(
        zikaron::hexfmt::encode(&got.content),
        zikaron::hexfmt::encode(&zikaron::cryptox::sha256(&bytes.stdout)),
        "content 与 git cat-file 的字节对不上"
    );
    assert_eq!(got.bytes, bytes.stdout.len(), "提交对象的长度对不上");

    let rev = output_of(Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&repo)
        )
        .expect("起不动 git");
    assert_eq!(got.commit, String::from_utf8_lossy(&rev.stdout).trim(), "HEAD 指的那一枚对不上");

    let count = output_of(Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .current_dir(&repo)
        )
        .expect("起不动 git");
    assert_eq!(
        got.ancestors.to_string(),
        String::from_utf8_lossy(&count.stdout).trim(),
        "祖先数对不上"
    );

    let subj = output_of(Command::new("git")
        .args(["log", "-1", "--format=%s"])
        .current_dir(&repo)
        )
        .expect("起不动 git");
    assert_eq!(got.subject, String::from_utf8_lossy(&subj.stdout).trim(), "提交摘要对不上");
    assert_eq!(got.ancestors, 5, "五枚提交");
    let _ = std::fs::remove_dir_all(&repo);
}

/// A directory manifest is canonical: the same tree gives the same digest twice, and changing one byte
/// changes it.
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
    // An empty directory has no manifest: refused by name.
    let empty = dir.join("b2");
    std::fs::create_dir_all(&empty).expect("建目录");
    assert!(
        matches!(app::anchorx::of_dir(&empty), Err(f) if f.said().starts_with("DIR_EMPTY")),
        "空目录要具名拒"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The mode is always the family literal: `mark` is `bytes-sha256/1` and `toolchain` is the sha256 of its
/// UTF-8 bytes; the user can neither choose nor fill it.
#[test]
fn the_mode_is_the_family_literal_and_nothing_else() {
    let m = app::anchorx::mode();
    assert_eq!(m.mark, "bytes-sha256/1", "家族字面只此一员");
    assert_eq!(m.mark, app::anchorx::FAMILY);
    assert_eq!(m.toolchain, zikaron::cryptox::sha256(b"bytes-sha256/1"), "toolchain 取字面的 UTF-8 字节 sha256");
    // The body follows law §6.2's three cells and is judged by the core's checks, not by this layer.
    let body = app::anchorx::history_body(&[7u8; 32], &m, "");
    let zikaron::json::Value::Obj(ms) = &body else { panic!("body 该是一个对象") };
    let names: Vec<&str> = ms.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(names, vec!["content", "mode"], "没有附言的时候恰两格");
    // The literal is spelled in one place only.
    for f in std::fs::read_dir(src()).expect("读得出 src") {
        let p = f.expect("一档").path();
        if p.extension().map(|x| x == "rs").unwrap_or(false) && p.file_name().map(|n| n != "anchorx.rs").unwrap_or(false) {
            let code = code_only(&std::fs::read_to_string(&p).expect("读得出"));
            assert!(!code.contains("bytes-sha256/1"), "{} 里自拼了家族字面", p.display());
        }
    }
}

/// The queue has one way in and two ways out (a receipt via `anchored_out`, a retraction via `drop_ids`),
/// and its file on disk is a canonical value.
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

/// The depth reading and the grantee verifier share one implementation: `depthx::read` and the kit crate's
/// `reading::depth` produce identical canonical bytes.
#[test]
fn the_depth_reading_comes_from_the_kit_core_itself() {
    vault_open();
    let dir = std::env::temp_dir().join(format!("zk-depth-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = app::home::Home::open_or_create(&dir).expect("铺得出家");
    let secret = app::key::Secret::take([0x21u8; 32]).expect("在阶内");
    // A genesis plus one history entry, built through the app's own assembly.
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

    // This page writes no external report file (kit law §9): `depthx` has no path to disk.
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

    // Round trip to disk: a canonical value, read back identically.
    let dir = std::env::temp_dir().join(format!("zk-wiz-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = app::home::Home::open_or_create(&dir).expect("铺得出家");
    w.write(&home).expect("落得下");
    assert_eq!(Wizard::read(&home).expect("读得回"), w, "落盘往返逐条相同");
    // A file whose order was scrambled by hand is refused (written through the same seal as real data).
    let put = |b: &[u8]| app::local::put(&home.dir(app::home::Slot::Settings), app::wizard::FILE, app::local::Doc::FirstWindow, b).expect("写得下");
    put(br#"{"marks":[{"said":"","step":"anchor"}]}"#);
    assert!(Wizard::read(&home).is_err(), "乱了次序的清单不许被读成一份清单");
    // Older files with lines for removed steps (`bond`, `undertaking`): those lines are skipped and the rest
    // continue in the current order.
    put(br#"{"marks":[{"said":"a","step":"terms"},{"said":"b","step":"bond"},{"said":"c","step":"undertaking"}]}"#);
    let old = Wizard::read(&home).expect("留着撤下那几步的旧档照读得开");
    assert_eq!((old.done(), old.next()), (1, Some(Step::Anchor)), "旧档停在原处");
    assert!(Step::parse("undertaking").is_none(), "撤下的那一步认不出");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The double-sale gate fires only for the same record, overlapping windows and the existing grant's local
/// exclusive flag (terms are only a hash, so exclusivity cannot be read from them, law §6.3).
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

    // The badge needs a chain time; without it the reading is "no reading", never a guessed "within the window".
    let r = row(false, Some((10, 20)));
    assert_eq!(r.badge(None), app::grantx::Badge::Unknown);
    assert_eq!(r.badge(Some(15)), app::grantx::Badge::Live);
    assert_eq!(r.badge(Some(99)), app::grantx::Badge::Expired);
    let mut rr = r.clone();
    rr.revoked = true;
    assert_eq!(rr.badge(Some(15)), app::grantx::Badge::Revoked, "撤了压过在窗");
}

// ═════════════════════ From revocation to reading ═════════════════════

/// A revocation must revoke a grant in this ledger, and "revoked" overrides "within the window" (law §6.4).
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

    // A time inside the window still reads "revoked".
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
    // A failed co-signature does not refuse the entry: a body without those two cells is still legal.
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
    // A succession to oneself is not a handover.
    assert!(handed_over(&pile.items, app::key::Address::parse(&to)).is_none());
    // Without a local key the question cannot be asked yet.
    assert!(handed_over(&pile.items, None).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

// ═════════════════════ Shell navigation · Settings ═════════════════════

/// The recorder rail has 6 items and the user rail 4 (delivery merged into the verify page), with the
/// designed groups and item names.
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

/// Every old page lands somewhere findable in both seats (a rail view, a settings section, or home for a view
/// belonging to the other seat), and the landing view lights one rail item.
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
                    // The user's relicense and the pending queue are entered from links; the rail lights nothing.
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

/// The language setting is saved to settings and read back after reopening; without a home it is refused by
/// name and the language does not switch.
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

/// A home's settings keep members this version does not know: a member written by a newer version is written
/// back unchanged when this version saves.
#[test]
fn a_homes_settings_keep_what_this_version_does_not_know() {
    vault_open();
    use app::action::{apply, Action, Applied};
    let dir = std::env::temp_dir().join(format!("zk-test-extra-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let home = app::home::Home::open_or_create(&dir).expect("建家");
    shell.lock = Some(app::lock::take(&home).expect("取锁"));
    let room = home.dir(app::home::Slot::Settings);
    let later = b"{\"capBytes\":1048576,\"futureCell\":{\"x\":[1,2]},\"role\":\"author\"}";
    app::local::put(&room, app::settings::FILE, app::local::Doc::Settings, later).expect("落一份新版的设置");
    shell.home = Some(home);
    shell.settings = app::settings::Settings::read(shell.home.as_ref().unwrap()).expect("读");
    match apply(&mut shell, Action::SetHideLocalDeletions { on: true }) {
        Applied::HideLocalDeletions(true) => {}
        other => panic!("该存下:{other:?}"),
    }
    let raw = app::local::read(&room.join(app::settings::FILE), app::local::Doc::Settings).expect("读").expect("在");
    let text = String::from_utf8(raw).expect("utf-8");
    assert!(text.contains("\"futureCell\":{\"x\":[1,2]}") && text.contains("\"hideLocalDeletions\":true"), "{text}");
    drop(shell);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A home's settings are read leniently: a seat other than exactly "grantee" is the author's, a capacity of 0
/// is the default, and a non-text endpoint is dropped (the rest of the list is kept).
#[test]
fn a_homes_settings_are_read_wide() {
    vault_open();
    let dir = std::env::temp_dir().join(format!("zk-test-wide-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = app::home::Home::open_or_create(&dir).expect("建家");
    let room = home.dir(app::home::Slot::Settings);
    let later = b"{\"capBytes\":0,\"endpoints\":[\"1=https://a.example\",5],\"role\":\"Grantee\"}";
    app::local::put(&room, app::settings::FILE, app::local::Doc::Settings, later).expect("落一份设置");
    let s = app::settings::Settings::read(&home).expect("读");
    assert_eq!((s.role, s.cap_bytes, s.endpoints.clone()), (app::roles::Role::Author, app::settings::CAP_DEFAULT, vec!["1=https://a.example".to_string()]));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Re-run the calling test in a process of its own, so its machine directory (keyed by process) is its alone;
/// needed by tests that restore a backup, which replaces the vault. Returns `true` in the parent (after
/// checking the child passed) and `false` in the child, which runs the body.
fn alone(test: &str) -> bool {
    const CHILD: &str = "ZK_CHECKS_ALONE";
    if std::env::var(CHILD).as_deref() == Ok(test) {
        return false;
    }
    let me = std::env::current_exe().expect("this test binary");
    let out = output_of(std::process::Command::new(me).args([test, "--exact", "--nocapture", "--test-threads=1"]).env(CHILD, test)).expect("the child runs");
    assert!(out.status.success() && String::from_utf8_lossy(&out.stdout).contains("1 passed"), "{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    true
}

/// What came after the last whole-machine backup is counted one way everywhere (`machine::backup_behind`),
/// including older or foreign indexes and after a restore; the read-only networks table round-trips.
#[test]
fn what_came_after_the_backup_is_behind_it() {
    if alone("what_came_after_the_backup_is_behind_it") {
        return;
    }
    vault_open();
    use app::action::{apply, Action, Applied};
    let base = std::env::temp_dir().join(format!("zk-behind-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let machine_dir = app::home::machine_dir().expect("机器目录");
    let behind = |m: &app::machine::Machine| app::machine::backup_behind(m.backup.as_ref(), app::backup::measured(m.backup.as_ref()).ok());
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    assert!(matches!(apply(&mut shell, Action::MakeAnchorKey), Applied::AnchorKey(_)));
    // A home with one entry of its own (the first entry is recorded on the spot; no task to wait for).
    let home = |shell: &mut app::shell::Shell, name: &str| {
        let dir = base.join(name);
        assert!(matches!(apply(shell, Action::OpenHome { root: dir.display().to_string() }), Applied::Homed { .. }));
        let made = apply(shell, Action::Genesis { statement: format!("{name} 的开端") });
        assert!(matches!(made, Applied::Genesised { .. }), "{made:?}");
        dir
    };
    home(&mut shell, "a");
    let b = home(&mut shell, "b");
    let net = app::readnets::cells("", "11155420", &format!("0x{}", "11".repeat(20)), "0", "https://read.example").expect("一行只读网络");
    app::readnets::write(&machine_dir, &[net.clone()]).expect("落只读网络表");

    let first = app::backup::export(&base.join("out"), "zikaron-backup-probe", 1_800_000_000).expect("备份");
    let m = app::machine::read().expect("机器设置");
    let last = m.backup.clone().expect("记下了这一次备份");
    assert!(last.indexed && first.path.exists() && last.count == 2, "{last:?}");
    assert_eq!(behind(&m), Some(0), "刚备份");

    home(&mut shell, "c");
    assert_eq!(behind(&m), Some(1), "备份之后添一处、记一条");

    // A home whose folder is deleted: its content is in the backup, and it no longer offsets later entries.
    std::fs::remove_dir_all(&b).expect("删去一处数据夹");
    assert_eq!(behind(&m), Some(1), "删去一处之后");
    home(&mut shell, "d");
    assert_eq!(behind(&m), Some(2), "删去一处之后再记一条");

    // An older backup record (no index): the old count, until the next backup (3 now against 2: one, not two).
    let now = app::backup::count_now().expect("现数");
    let older = app::machine::Backed { indexed: false, ..last.clone() };
    assert_eq!(app::backup::measured(Some(&older)).ok(), Some(now));
    assert_eq!(app::machine::backup_behind(Some(&older), Some(now)), Some(1));
    // An index that does not read: the old count.
    let index_at = app::backup::index_path().expect("索引处");
    let index = std::fs::read(&index_at).expect("索引在");
    std::fs::write(&index_at, b"{not an index").expect("坏索引");
    assert_eq!(app::backup::measured(Some(&last)).ok(), Some(now), "索引读不成");
    std::fs::write(&index_at, &index).expect("放回");
    // Another backup's index (same items, different backup time), sealed as the app seals it: the old count.
    let text = String::from_utf8(app::local::read(&index_at, app::local::Doc::BackupIndex).expect("开得了").expect("索引在")).expect("索引是文字");
    let other = text.replacen("\"at\":1800000000", "\"at\":1800000100", 1);
    assert_ne!(other, text);
    let room = index_at.parent().expect("索引屋");
    app::local::put(room, app::backup::INDEX_FILE, app::local::Doc::BackupIndex, other.as_bytes()).expect("别次的索引");
    assert_eq!(app::backup::measured(Some(&last)).ok(), Some(now), "别次备份的索引");
    std::fs::write(&index_at, &index).expect("放回");
    assert_eq!(behind(&m), Some(2), "放回之后照按索引算");

    // Restored from the first backup: nothing behind and the table restored; then one more entry is behind.
    std::fs::remove_file(app::readnets::path_in(&machine_dir)).expect("删只读网络表");
    drop(shell);
    app::backup::restore(&first.path, "zikaron-backup-probe", app::backup::From::Settings("27618394")).expect("恢复");
    let m = app::machine::read().expect("恢复后的机器设置");
    assert_eq!(m.backup.as_ref().map(|b| (b.at, b.indexed)), Some((1_800_000_000, true)));
    assert_eq!(behind(&m), Some(0), "恢复之后");
    // The restored machine's index is sealed under the new key; no plain index sits beside the settings.
    let restored_index = std::fs::read(app::backup::index_path().expect("索引处")).expect("恢复写下了索引");
    assert!(app::local::is_sealed(&restored_index) && restored_index != index, "恢复之后的索引封在新钥下");
    assert!(!machine_dir.join(app::backup::PLAIN_INDEX_FILE).exists());
    assert_eq!(app::readnets::read(&machine_dir).expect("表落回"), vec![net.clone()]);
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    // The anchoring key belongs to this machine, not the backup: it is made again for the restored vault.
    assert!(matches!(apply(&mut shell, Action::MakeAnchorKey), Applied::AnchorKey(_)));
    home(&mut shell, "e");
    assert_eq!(behind(&m), Some(1), "恢复之后再记一条");
    drop(shell);
    let _ = std::fs::remove_dir_all(&base);
}

/// A backup without the read-only networks table (as written by older versions) opens and restores, and
/// leaves this machine's table as it is. Runs in its own process ([`alone`]).
#[test]
fn a_backup_without_the_table_leaves_this_machines_table() {
    if alone("a_backup_without_the_table_leaves_this_machines_table") {
        return;
    }
    vault_open();
    use app::action::{apply, Action, Applied};
    let base = std::env::temp_dir().join(format!("zk-bare-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let machine_dir = app::home::machine_dir().expect("机器目录");
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    assert!(matches!(apply(&mut shell, Action::MakeAnchorKey), Applied::AnchorKey(_)));
    assert!(matches!(apply(&mut shell, Action::OpenHome { root: base.join("a").display().to_string() }), Applied::Homed { .. }));
    assert!(matches!(apply(&mut shell, Action::Genesis { statement: "a".into() }), Applied::Genesised { .. }));
    assert!(!app::readnets::path_in(&machine_dir).exists());
    let bare = app::backup::export(&base.join("out"), "zikaron-backup-probe", 1_800_000_200).expect("无表的备份");
    let net = app::readnets::cells("", "11155420", &format!("0x{}", "11".repeat(20)), "0", "https://read.example").expect("一行只读网络");
    app::readnets::write(&machine_dir, &[net.clone()]).expect("备份之后落表");
    drop(shell);
    app::backup::restore(&bare.path, "zikaron-backup-probe", app::backup::From::Settings("27618394")).expect("恢复无表的备份");
    assert_eq!(app::readnets::read(&machine_dir).expect("表照旧"), vec![net]);
    let _ = std::fs::remove_dir_all(&base);
}

/// The proxy choice is saved by one action: `system`, `none`, or a parsed address in canonical spelling;
/// unparseable addresses and credentials are refused and nothing is written.
#[test]
fn the_proxy_choice_is_saved_as_read_or_refused_untouched() {
    vault_open();
    use app::action::{apply, Action, Applied};
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let on_disk = || std::fs::read(app::machine::path().expect("机器设置的路")).unwrap_or_default();
    for (typed, written) in [
        ("system", "system"),
        ("  none  ", "none"),
        (" SOCKS5://Proxy.Example:1080/ ", "socks5://proxy.example:1080"),
        ("http://[::1]:7890", "http://[::1]:7890"),
        ("HTTP://127.0.0.1:8080", "http://127.0.0.1:8080"),
    ] {
        match apply(&mut shell, Action::SetProxy { choice: typed.into() }) {
            Applied::ProxySet(w) => assert_eq!(w, written, "{typed:?}"),
            other => panic!("{typed:?}: {other:?}"),
        }
        assert_eq!(app::machine::read().expect("读").proxy.as_deref(), Some(written));
    }
    for (typed, tail_key) in [
        ("proxy.example:8080", app::lang::Key::TailProxyShape),
        ("https://127.0.0.1:1", app::lang::Key::TailProxyShape),
        ("http://127.0.0.1", app::lang::Key::TailProxyShape),
        ("", app::lang::Key::TailProxyShape),
        ("http://u:p@127.0.0.1:1", app::lang::Key::TailProxyCredentials),
    ] {
        let before = on_disk();
        match apply(&mut shell, Action::SetProxy { choice: typed.into() }) {
            Applied::Trouble(f) => {
                assert!(f.said().starts_with("SETTINGS_SHAPE"), "{typed:?}: {}", f.said());
                let tails: Vec<String> = app::lang::TABLE.iter().filter(|(k, _, _)| *k == tail_key).flat_map(|(_, zh, en)| [zh.to_string(), en.to_string()]).collect();
                assert!(tails.iter().any(|t| f.tail() == t.replace("{0}", typed)), "{typed:?}: {}", f.tail());
            }
            other => panic!("{typed:?}: {other:?}"),
        }
        assert_eq!(on_disk(), before, "{typed:?}: refused, not one byte moved");
    }
}

/// The proxy line on the settings page says how a new connection goes and, when it goes direct, why.
#[test]
fn the_proxy_line_says_the_way_and_why_it_is_straight() {
    use zikaron_net::{Choice, Kind, Proxy, Reading, Way};
    let texts = |k: app::lang::Key| -> Vec<String> { app::lang::TABLE.iter().filter(|(x, _, _)| *x == k).flat_map(|(_, zh, en)| [zh.to_string(), en.to_string()]).collect() };
    let plain = Reading { way: Way::Direct, choice: Choice::System, loopback: false, auto_config_ignored: false, system_unread: false };
    let p = Proxy { kind: Kind::Socks5, host: "127.0.0.1".into(), port: 1080 };
    let via = app::machine::proxy_said(&Reading { way: Way::Through(p), choice: Choice::Off, ..plain.clone() });
    assert!(texts(app::lang::Key::ProxyNowVia).iter().any(|t| via == t.replace("{0}", "socks5://127.0.0.1:1080")), "{via}");
    for (r, k) in [
        (Reading { loopback: true, choice: Choice::Manual(Proxy { kind: Kind::Http, host: "p".into(), port: 1 }), ..plain.clone() }, app::lang::Key::ProxyNowLoopback),
        (Reading { system_unread: true, ..plain.clone() }, app::lang::Key::ProxyNowUnread),
        (Reading { auto_config_ignored: true, ..plain.clone() }, app::lang::Key::ProxyNowAutoConfig),
        (plain.clone(), app::lang::Key::ProxyNowDirect),
    ] {
        let said = app::machine::proxy_said(&r);
        assert!(texts(k).contains(&said), "{k:?}: {said}");
    }
}

/// A JSON-RPC node on a local port answering each request with `answer(method)` (`None`: close the
/// connection unanswered), one request per connection.
fn rpc_node(answer: fn(&str) -> Option<String>) -> String {
    use std::io::{Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            std::thread::spawn(move || {
                let mut s = s;
                let mut raw = Vec::new();
                let mut buf = [0u8; 4096];
                let body = loop {
                    match s.read(&mut buf) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => raw.extend_from_slice(&buf[..n]),
                    }
                    let text = String::from_utf8_lossy(&raw).to_string();
                    if let Some(i) = text.find("\r\n\r\n") {
                        let len: usize = text[..i].lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").and_then(|n| n.trim().parse().ok())).unwrap_or(0);
                        if raw.len() >= i + 4 + len {
                            break text[i + 4..].to_string();
                        }
                    }
                };
                let method = body.split("\"method\":\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string();
                let id = body.split("\"id\":").nth(1).and_then(|r| r.split(|c| c == ',' || c == '}').next()).unwrap_or("1").trim().to_string();
                let Some(result) = answer(&method) else { return };
                let (status, body) = if let Some(code) = result.strip_prefix("HTTP ") {
                    (code.to_string(), String::from("{}"))
                } else {
                    ("200 OK".to_string(), format!("{{\"id\":{id},\"jsonrpc\":\"2.0\",\"result\":{result}}}"))
                };
                let _ = s.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            });
        }
    });
    url
}

/// The contract code the gate tests pin: `0x60` (its keccak is the pin).
fn test_pin() -> String {
    zikaron::hexfmt::encode(&zikaron::cryptox::keccak256(&[0x60]))
}

fn net_of(nodes: Vec<String>) -> app::readnets::Net {
    app::readnets::Net { chain_id: 31337, registry: app::key::Address([0x11; 20]), from_block: 0, nodes, name: None }
}

/// The read-only networks' registry gate: nodes returning the pinned code count, any other code is a
/// mismatch, and with no counting node the first trouble is named with every node's words in the tail.
#[test]
fn the_registry_gate_counts_only_the_pinned_code_and_says_why_when_none_counts() {
    use app::widex::{gate_said_against, Reading};
    let pin = test_pin();
    let pinned = rpc_node(|m| Some(match m { "eth_chainId" => "\"0x7a69\"".into(), "eth_getCode" => "\"0x60\"".into(), _ => "null".into() }));
    let pinned2 = rpc_node(|m| Some(match m { "eth_chainId" => "\"0x7a69\"".into(), "eth_getCode" => "\"0x60\"".into(), _ => "null".into() }));
    let other_code = rpc_node(|m| Some(match m { "eth_chainId" => "\"0x7a69\"".into(), "eth_getCode" => "\"0x61\"".into(), _ => "null".into() }));
    let other_chain = rpc_node(|m| Some(match m { "eth_chainId" => "\"0x1\"".into(), _ => "\"0x60\"".into() }));
    let no_code = rpc_node(|m| Some(match m { "eth_chainId" => "\"0x7a69\"".into(), _ => "null".into() }));
    let limited = rpc_node(|_| Some("HTTP 429 Too Many Requests".into()));
    let silent = rpc_node(|_| None);
    let dead = { let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind"); format!("http://{}", l.local_addr().expect("addr")) };
    let read = |nodes: Vec<&String>| gate_said_against(&net_of(nodes.into_iter().cloned().collect()), &pin);
    assert_eq!(read(vec![&pinned]).0, Reading::Single);
    assert_eq!(read(vec![&pinned, &pinned2]).0, Reading::Agreed(2));
    assert_eq!(read(vec![&pinned, &other_code]).0, Reading::Fingerprint, "one node with other code is a mismatch");
    assert_eq!(read(vec![&other_code, &pinned]).0, Reading::Fingerprint);
    assert_eq!(read(vec![&pinned, &dead, &other_chain]).0, Reading::Single, "a node that does not count does not stop the others");
    for (nodes, k, words) in [
        (vec![&other_chain], "UNREACHABLE", "eth_chainId 0x1"),
        (vec![&no_code], "UNREACHABLE", "answered no code"),
        (vec![&limited], "RATE_LIMITED", "429"),
        (vec![&dead], "UNREACHABLE", "127.0.0.1"),
        (vec![&silent], "UNREACHABLE", "127.0.0.1"),
    ] {
        let (reading, said) = read(nodes);
        let said = said.expect("said why");
        assert_eq!(reading, Reading::Down);
        assert!(said.said().starts_with(k) && said.tail().contains(words), "{k}: {} · {}", said.said(), said.tail());
    }
    let (reading, said) = read(Vec::new());
    assert_eq!((reading, said.map(|f| f.said().starts_with("UNREACHABLE"))), (Reading::Down, Some(true)), "no node at all");
    assert_eq!(app::widex::gate(&net_of(vec![pinned.clone()])), Reading::Fingerprint, "the product's own pin: 0x60 is not the pinned build");
}

/// Saving the main network's custom settings: shapes are checked at once, and with nodes for that chain the
/// registry must be the pinned build before anything is written; without nodes the cells read "not checked".
#[test]
fn the_main_networks_registry_is_checked_when_saved_and_written_only_when_pinned() {
    vault_open();
    use app::action::{apply, Action, Applied};
    let dir = std::env::temp_dir().join(format!("zk-test-basis-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let home = app::home::Home::open_or_create(&dir).expect("建家");
    shell.lock = Some(app::lock::take(&home).expect("取锁"));
    shell.home = Some(home);
    let reg = format!("0x{}", "11".repeat(20));
    let basis = |chain: &str, registry: &str, from: &str| Action::SetBasis { chain: chain.into(), registry: registry.into(), from_block: from.into() };
    let settled = |shell: &mut app::shell::Shell| {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while shell.tasks.in_flight(app::task::Kind::Basis) && std::time::Instant::now() < until {
            shell.drain();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        shell.drain();
    };
    // Each cell's shape is checked at once.
    for (a, k) in [(basis("x", &reg, "0"), "SETTINGS_SHAPE"), (basis("31337", "0x12", "0"), "ADDRESS_SHAPE"), (basis("31337", &reg, "-1"), "SETTINGS_SHAPE")] {
        match apply(&mut shell, a) {
            Applied::Trouble(f) => assert!(f.said().starts_with(k), "{k}: {}", f.said()),
            other => panic!("{k}: {other:?}"),
        }
    }
    assert_eq!(shell.settings.chain_id, None);
    // A node whose registry code is not the pinned build: checked, reported, not written.
    let other = rpc_node(|m| Some(match m { "eth_chainId" => "\"0x7a69\"".into(), "eth_getCode" => "\"0x60\"".into(), _ => "null".into() }));
    assert!(matches!(apply(&mut shell, Action::SetEndpoints { specs: format!("31337={other}") }), Applied::Endpoints(_)));
    assert!(matches!(apply(&mut shell, basis("31337", &reg, "0")), Applied::Started(app::task::Kind::Basis)));
    settled(&mut shell);
    assert_eq!(shell.basis_read.map(|(_, _, r)| r), Some(Some(app::widex::Reading::Fingerprint)));
    assert_eq!(shell.settings.chain_id, None, "a mismatch is not written");
    // No node answering: the named fault, not written.
    let dead = { let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind"); format!("http://{}", l.local_addr().expect("addr")) };
    assert!(matches!(apply(&mut shell, Action::SetEndpoints { specs: format!("31337={dead}") }), Applied::Endpoints(_)));
    let before = shell.faults.len();
    assert!(matches!(apply(&mut shell, basis("31337", &reg, "0")), Applied::Started(app::task::Kind::Basis)));
    settled(&mut shell);
    assert_eq!(shell.basis_read.map(|(_, _, r)| r), Some(Some(app::widex::Reading::Down)));
    assert!(shell.faults[before..].iter().any(|f| f.said().starts_with("UNREACHABLE")), "said by name");
    assert_eq!(shell.settings.chain_id, None, "not checked is not checked");
    // An admitting landing writes the three cells.
    let asked = app::action::basis_stamp(&shell, 31337);
    app::action::basis_read(&mut shell, 31337, app::key::Address([0x11; 20]), 7, app::widex::Reading::Single, None, false, asked);
    assert_eq!((shell.settings.chain_id, shell.settings.registry, shell.settings.from_block), (Some(31337), Some(app::key::Address([0x11; 20])), 7));
    // No node for that chain yet: written as typed, read "not checked".
    assert!(matches!(apply(&mut shell, Action::SetEndpoints { specs: String::new() }), Applied::Endpoints(0)));
    let later = app::key::Address([0x22; 20]);
    match apply(&mut shell, basis("31337", &later.hex(), "3")) {
        Applied::Basis { chain } => assert_eq!(chain, 31337),
        other => panic!("{other:?}"),
    }
    assert_eq!((shell.settings.chain_id, shell.settings.registry, shell.settings.from_block), (Some(31337), Some(later), 3));
    assert_eq!(shell.basis_read, Some((31337, later, None)), "not checked, never shown as checked");
    assert!(!shell.tasks.in_flight(app::task::Kind::Basis), "no node: nothing asked");
    // A value the settings cannot hold, with no node: refused by name at once, nothing written.
    match apply(&mut shell, basis("9007199254740992", &later.hex(), "3")) {
        Applied::Trouble(f) => assert!(f.said().starts_with("SETTINGS_SHAPE"), "{}", f.said()),
        other => panic!("{other:?}"),
    }
    assert_eq!(shell.settings.chain_id, Some(31337));
    // Nodes for that chain saved afterwards: the same check runs, the cells stay either way, and a fingerprint
    // other than the pinned build shows beside them.
    assert!(matches!(apply(&mut shell, Action::SetEndpoints { specs: format!("31337={other}") }), Applied::Endpoints(1)));
    assert!(shell.tasks.in_flight(app::task::Kind::Basis), "the check runs again on the new nodes");
    assert_eq!(shell.basis_read, Some((31337, later, None)), "still not checked while the check is out");
    settled(&mut shell);
    assert_eq!(shell.basis_read, Some((31337, later, Some(app::widex::Reading::Fingerprint))));
    assert_eq!((shell.settings.chain_id, shell.settings.registry, shell.settings.from_block), (Some(31337), Some(later), 3), "saving nodes takes nothing away");
    assert!(matches!(apply(&mut shell, Action::SetEndpoints { specs: format!("31337={dead}") }), Applied::Endpoints(1)));
    let before = shell.faults.len();
    settled(&mut shell);
    assert_eq!(shell.basis_read, Some((31337, later, Some(app::widex::Reading::Down))));
    assert_eq!(shell.faults.len(), before, "nodes not answering are read beside the cell, not said again");
    assert_eq!(shell.settings.registry, Some(later));
    // Nodes for another chain only: nothing queried.
    assert!(matches!(apply(&mut shell, Action::SetEndpoints { specs: format!("1={dead}") }), Applied::Endpoints(1)));
    assert!(!shell.tasks.in_flight(app::task::Kind::Basis));
    // An admitting check landing after nodes: the reading only, nothing rewritten; one about cells changed
    // meanwhile is dropped.
    let asked = app::action::basis_stamp(&shell, 31337);
    app::action::basis_read(&mut shell, 31337, later, 99, app::widex::Reading::Single, None, true, asked);
    assert_eq!((shell.settings.from_block, shell.basis_read), (3, Some((31337, later, Some(app::widex::Reading::Single)))));
    let asked = app::action::basis_stamp(&shell, 31337);
    app::action::basis_read(&mut shell, 31337, app::key::Address([0x33; 20]), 3, app::widex::Reading::Fingerprint, None, true, asked);
    assert_eq!(shell.basis_read, Some((31337, later, Some(app::widex::Reading::Single))), "another cell's reading is dropped");
    // Another setting written meanwhile (the audit interval) leaves the reading standing.
    let asked = app::action::basis_stamp(&shell, 31337);
    shell.commit_settings(|s| s.audit_every += 1).expect("an unrelated setting saved");
    assert!(app::action::basis_read(&mut shell, 31337, later, 3, app::widex::Reading::Single, None, true, asked), "an unrelated write does not drop it");
    // A reading for a main network other than the current one applies to nothing.
    let mut asked = app::action::basis_stamp(&shell, 31337);
    asked.cell ^= 1;
    app::action::basis_read(&mut shell, 31337, later, 3, app::widex::Reading::Fingerprint, None, true, asked);
    let taken = shell.basis_read;
    assert!(taken.is_none_or(|(_, _, r)| r != Some(app::widex::Reading::Fingerprint)), "a reading asked for earlier settings is dropped: {taken:?}");
    assert_eq!((shell.settings.chain_id, shell.settings.registry, shell.settings.from_block), (Some(31337), Some(later), 3), "and writes nothing");
    // A built-in deployment: saved at once, no node queried.
    let d = app::deploy::KNOWN[0];
    match apply(&mut shell, basis(&d.chain_id.to_string(), d.registry, &d.from_block.to_string())) {
        Applied::Basis { chain } => assert_eq!(chain, d.chain_id),
        other => panic!("{other:?}"),
    }
    assert_eq!(shell.settings.chain_id, Some(d.chain_id));
    // Nodes saved for a built-in row: nothing queried (the app's own pin applies).
    assert!(matches!(apply(&mut shell, Action::SetEndpoints { specs: format!("{}={dead}", d.chain_id) }), Applied::Endpoints(1)));
    assert!(!shell.tasks.in_flight(app::task::Kind::Basis));
    drop(shell);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The check page validates typed cells' shapes (registry first) before reporting what is missing.
#[test]
fn the_check_page_judges_each_typed_cell_before_what_is_missing() {
    use app::action::{apply, Action, Applied};
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let check = |endpoints: &str, registry: &str, from: &str| Action::CheckPayload {
        typed: "0x01".into(),
        ledgers: String::new(),
        endpoints: endpoints.into(),
        registry: registry.into(),
        from_block: from.into(),
        now: String::new(),
        file: String::new(),
        terms: String::new(),
    };
    let reg = format!("0x{}", "11".repeat(20));
    for (a, k, form) in [
        (check("", "", "x"), "SETTINGS_SHAPE", "a start block that does not read, no registry configured"),
        (check("", "", "-1"), "SETTINGS_SHAPE", "a negative start block"),
        (check("", "", "18446744073709551616"), "SETTINGS_SHAPE", "a start block past 64 bits"),
        (check("", "0x12", ""), "ADDRESS_SHAPE", "a registry that does not read"),
        (check("", "0x12", "x"), "ADDRESS_SHAPE", "both wrong: the registry first"),
        (check("not a node", &reg, "0"), "SETTINGS_SHAPE", "a node line that does not read"),
    ] {
        match apply(&mut shell, a) {
            Applied::Trouble(f) => assert!(f.said().starts_with(k), "{form}: {}", f.said()),
            other => panic!("{form}: {other:?}"),
        }
    }
    for (a, form) in [(check("", "", ""), "nothing typed, nothing configured"), (check("", &reg, ""), "a registry typed, the start block left")] {
        assert!(matches!(apply(&mut shell, a), Applied::Started(app::task::Kind::Check)), "{form}");
        let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while shell.tasks.in_flight(app::task::Kind::Check) && std::time::Instant::now() < until {
            shell.drain();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        shell.drain();
    }
}

/// The send card notes a fallback fee cap based on where the fees came from, never on their numbers.
#[test]
fn the_send_card_says_when_its_cap_is_the_fallback() {
    use zikaron_anchor::send::Fees;
    let fallback = Fees::fallback();
    assert_eq!(app::chainx::fee_source_line(None), Some(app::lang::Key::U3FeeFallback), "no fees read");
    assert_eq!(app::chainx::fee_source_line(Some(&fallback)), Some(app::lang::Key::U3FeeFallback), "the fallback pair");
    let same_numbers = Fees { from_chain: true, ..fallback };
    assert_eq!(app::chainx::fee_source_line(Some(&same_numbers)), None, "from the chain, though the numbers are the same");
}

/// The app's endpoint cell parses exactly like the shared parser (`rpc::endpoint_spec`, also used by the
/// command line): the same forms accepted, the same refused.
#[test]
fn the_app_reads_an_endpoint_spelling_as_the_command_line_does() {
    for s in [" 1=https://a.example", "1= https://a.example", "01=https://a.example", "+1=https://a.example", "1=https://a/?k=v", "1==x", "1", "=x", "-1=x", "1=", "1=  ", "18446744073709551616=x"] {
        let app = app::chainx::Endpoint::parse(s).map(|e| (e.chain, e.url));
        assert_eq!(app, zikaron_anchor::rpc::endpoint_spec(s).ok().map(|(c, u)| (c, app::chainx::NodeAddr::new(u))), "{s:?}");
    }
}

/// Set (or, with `None`, remove) one member at a path in a JSON object (tests mutate a well-formed file).
fn with_member(v: &zikaron::json::Value, path: &[&str], to: Option<zikaron::json::Value>) -> zikaron::json::Value {
    use zikaron::json::Value;
    let Value::Obj(m) = v else { return v.clone() };
    let mut m = m.clone();
    match path {
        [k] => {
            m.retain(|(x, _)| x != k);
            if let Some(t) = to {
                m.push((k.to_string(), t));
            }
        }
        [k, rest @ ..] => {
            for (x, inner) in m.iter_mut() {
                if x == k {
                    *inner = with_member(inner, rest, to.clone());
                }
            }
        }
        [] => {}
    }
    Value::Obj(m)
}

/// An unparseable node address is refused as an address-shape error, saying why and naming the port, on every
/// path that opens nodes; beside another node that is merely down, the network refusal applies as before.
#[test]
fn a_node_address_that_does_not_read_is_its_shape_naming_the_port() {
    use app::chainx::{address_said, head_block, Endpoint};
    let said_k = |k: app::lang::Key, x: &str| -> Vec<String> { app::lang::TABLE.iter().filter(|(y, _, _)| *y == k).flat_map(|(_, zh, en)| [zh.replace("{0}", x), en.replace("{0}", x)]).collect() };
    // The address is named the way every sentence names a node (`zikaron_net::sayable`): an unparseable one by
    // its length, never echoed (a key typed into it would leak).
    let named = |u: &str| zikaron_net::sayable(u);
    assert!(said_k(app::lang::Key::TailNodePort, &named("https://h:65536")).contains(&address_said("https://h:65536")));
    assert!(said_k(app::lang::Key::TailNodePort, &named("https://h:")).contains(&address_said("https://h:")));
    assert!(said_k(app::lang::Key::Tail093, &named("wss://h")).contains(&address_said("wss://h")));
    assert!(said_k(app::lang::Key::TailNodeAddress, &named("https://u@h")).contains(&address_said("https://u@h")));
    assert!(!address_said("https://u:S3CRET@h").contains("S3CRET"));
    assert_eq!(address_said("https://h:65535"), "");
    let ep = |u: &str| Endpoint { chain: 1, url: u.into() };
    match head_block(&[ep("https://h:65536"), ep("https://h:")], 1) {
        Err(f) => assert!(f.said().starts_with("SETTINGS_SHAPE") && f.tail().contains(&zikaron_net::sayable("https://h:65536")) && f.tail().contains(&zikaron_net::sayable("https://h:")), "{} {}", f.said(), f.tail()),
        Ok(_) => panic!("read"),
    }
    let dead = { let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind"); format!("http://{}", l.local_addr().expect("addr")) };
    match head_block(&[ep("https://h:65536"), ep(&dead)], 1) {
        Err(f) => assert!(!f.said().starts_with("SETTINGS_SHAPE"), "a node merely down beside it: the network's: {}", f.said()),
        Ok(_) => panic!("read"),
    }
}

/// Text written as an address (a scheme and `://`) is always a remote address, never a local path: an
/// out-of-range or empty port is refused naming the port; another scheme is refused by its own words.
#[test]
fn text_written_as_an_address_is_the_remote_familys() {
    use app::fetchx::{base_of, is_address};
    for a in ["https://localhost:65536/kit/", "https://localhost:/kit/", "ftp://x/", "HTTPS://h/", "git+ssh://h/x"] {
        assert!(is_address(a), "{a}");
    }
    for p in ["/local/kit", "kit", "C:\\kit", "./a://b", "://x", "1http://x"] {
        assert!(!is_address(p), "{p}");
    }
    for (a, tail) in [("https://localhost:65536/kit/", "65536"), ("https://localhost:/kit/", "localhost:"), ("ftp://x/", "ftp://x/")] {
        match base_of(a) {
            Err(f) => assert!(f.said().starts_with("REMOTE_NOT_HTTPS") && f.tail().contains(tail), "{a}: {} {}", f.said(), f.tail()),
            Ok(_) => panic!("{a}"),
        }
    }
}

/// A malformed keystore (`mac`, `ciphertext`, `r` or `p` missing or malformed) is refused as a file-shape
/// error before any key derivation work.
#[test]
fn a_keystore_of_the_wrong_shape_is_said_as_that_before_any_work() {
    use zikaron::json::Value;
    let secret = app::key::Secret::take([0x42; 32]).expect("a key");
    let ks = app::keystore::encrypt(&secret, "pw-probe", app::keystore::Params::light(), 1_700_000_000).expect("encrypted");
    let v = zikaron::json::parse(&ks.json).expect("json");
    let open = |v: &Value| app::keystore::decrypt(&zikaron::json::canon_bytes(v), "pw-probe").err().map(|f| f.said().split(':').next().unwrap_or("").to_string());
    assert_eq!(open(&v), None, "the file as written opens");
    for (form, path, to, want) in [
        ("mac absent", vec!["crypto", "mac"], None, "KEYSTORE_SHAPE"),
        ("mac empty", vec!["crypto", "mac"], Some(Value::Str(String::new())), "KEYSTORE_SHAPE"),
        ("mac a number", vec!["crypto", "mac"], Some(Value::Int(5)), "KEYSTORE_SHAPE"),
        ("mac 31 bytes", vec!["crypto", "mac"], Some(Value::Str("ab".repeat(31))), "KEYSTORE_SHAPE"),
        ("mac not hex", vec!["crypto", "mac"], Some(Value::Str("zz".repeat(32))), "KEYSTORE_SHAPE"),
        ("ciphertext absent", vec!["crypto", "ciphertext"], None, "KEYSTORE_SHAPE"),
        ("r absent", vec!["crypto", "kdfparams", "r"], None, "KEYSTORE_SHAPE"),
        ("r as text", vec!["crypto", "kdfparams", "r"], Some(Value::Str("8".into())), "KEYSTORE_SHAPE"),
        ("p absent", vec!["crypto", "kdfparams", "p"], None, "KEYSTORE_SHAPE"),
        ("r zero", vec!["crypto", "kdfparams", "r"], Some(Value::Int(0)), "KEYSTORE_PARAMS"),
        ("mac of another", vec!["crypto", "mac"], Some(Value::Str("ab".repeat(32))), "BAD_PASSWORD"),
    ] {
        assert_eq!(open(&with_member(&v, &path, to)).as_deref(), Some(want), "{form}");
    }
    let long = app::secret::Secret::from("x".repeat(1025));
    assert!(app::keystore::decrypt_typed(&ks.json, &long).err().map(|f| f.said().starts_with("PASSWORD_LONG")).unwrap_or(false), "past the cap on reading");
    let at_cap = app::secret::Secret::from("x".repeat(1024));
    assert!(app::keystore::decrypt_typed(&ks.json, &at_cap).err().map(|f| f.said().starts_with("BAD_PASSWORD")).unwrap_or(false), "at the cap: read, and wrong");
}

/// A whole number typed into an entry's cell is within the law's ceiling, or refused by one family: past the
/// ceiling (within 64 bits or beyond) says so; not a number says that.
#[test]
fn a_typed_whole_number_past_the_ceiling_is_one_family() {
    use app::fault::whole_within_ceiling;
    let k = app::lang::Key::Tail044;
    for (t, n) in [("0", 0), (" 05 ", 5), ("+7", 7), ("9007199254740991", 9007199254740991)] {
        assert_eq!(whole_within_ceiling(t, k).ok(), Some(n), "{t}");
    }
    let tail_of = |key: app::lang::Key| -> Vec<String> { app::lang::TABLE.iter().filter(|(y, _, _)| *y == key).flat_map(|(_, zh, en)| [zh.to_string(), en.to_string()]).collect() };
    for t in ["9007199254740992", "18446744073709551615", "18446744073709551616", "+99999999999999999999"] {
        let f = whole_within_ceiling(t, k).expect_err(t);
        assert!(f.said().starts_with("SETTINGS_SHAPE") && tail_of(app::lang::Key::TailPastIntCeiling).iter().any(|w| f.tail() == w.replace("{0}", t)), "{t}: {}", f.tail());
    }
    for t in ["-1", "x", "1.5", ""] {
        let f = whole_within_ceiling(t, k).expect_err(t);
        assert!(f.said().starts_with("SETTINGS_SHAPE") && tail_of(k).iter().any(|w| f.tail() == w.replace("{0}", &format!("{:?}", t.trim()))), "{t}: {}", f.tail());
    }
    // The two entries that carry such numbers: a succession's effective time and a grant's window.
    let past = app::succeedx::succession_body(&format!("0x{}", "88".repeat(20)), app::succeedx::KIND_ROTATION, "9007199254740992", "x").expect_err("past");
    assert!(past.said().starts_with("SETTINGS_SHAPE") && past.tail().contains("9007199254740992"));
}

/// A grant chain longer than 64 hops is refused as a payload error naming the 65th hop; one of 64 hops
/// resolves; a hop not in the pool means the chain does not reach its root.
#[test]
fn a_grant_chain_past_sixty_four_hops_is_past_the_limit() {
    let secret = app::key::Secret::take([0x43; 32]).expect("a key");
    let genesis = app::entryx::genesis(&secret, "hops").expect("genesis");
    let mut pool = vec![genesis.bytes.clone()];
    let mut ids: Vec<String> = Vec::new();
    for i in 0..65u64 {
        let d = app::grantx::Draft {
            grantee: format!("0x{}", "33".repeat(20)),
            work: format!("0x{:064x}", i + 1),
            terms: format!("0x{}", "44".repeat(32)),
            upstream: ids.last().cloned().unwrap_or_default(),
            ..Default::default()
        };
        let body = app::grantx::grant_body(&d).expect("a body");
        let g = app::entryx::seal(&secret, "grant", 1, Some(&genesis.id), body).expect("a grant");
        pool.push(g.bytes);
        ids.push(g.id);
    }
    assert_eq!(app::badgex::chain_for(&pool, &ids[63]).map(|c| c.len()).ok(), Some(64), "64 hops cascade");
    let past = app::badgex::chain_for(&pool, &ids[64]).expect_err("65 hops");
    assert!(past.said().starts_with("PAYLOAD_REFUSED") && past.tail().contains(&ids[0]), "{} {}", past.said(), past.tail());
    let without_root: Vec<Vec<u8>> = pool.iter().filter(|b| zikaron::entry::check(b).map(|e| !e.id_hex().trim_start_matches("0x").eq_ignore_ascii_case(ids[0].trim_start_matches("0x"))).unwrap_or(true)).cloned().collect();
    let no_root = app::badgex::chain_for(&without_root, &ids[3]).expect_err("no root");
    assert!(no_root.said().starts_with("CHAIN_UNREACHED"), "{} {} ({} of {})", no_root.said(), no_root.tail(), without_root.len(), pool.len());
}

/// A malformed grant entry id is refused as a content-shape error before any lookup; a well-formed id not in
/// the pool is a chain not reaching its root.
#[test]
fn a_grant_named_by_an_id_of_the_wrong_shape_is_said_as_that_shape() {
    let secret = app::key::Secret::take([0x45; 32]).expect("a key");
    let genesis = app::entryx::genesis(&secret, "shape").expect("genesis");
    let d = app::grantx::Draft { grantee: format!("0x{}", "33".repeat(20)), work: format!("0x{:064x}", 1), terms: format!("0x{}", "44".repeat(32)), ..Default::default() };
    let g = app::entryx::seal(&secret, "grant", 1, Some(&genesis.id), app::grantx::grant_body(&d).expect("a body")).expect("a grant");
    let pool = vec![genesis.bytes.clone(), g.bytes.clone()];
    let id = zikaron::entry::check(&g.bytes).expect("an entry").id_hex();
    let bare = id.trim_start_matches("0x").to_string();
    for (form, typed) in [
        ("empty", String::new()),
        ("white space only", "   ".to_string()),
        ("the prefix alone", "0x".to_string()),
        ("one digit short", id[..id.len() - 1].to_string()),
        ("one digit long", format!("{id}0")),
        ("no prefix", bare.clone()),
        ("the prefix in capitals", format!("0X{bare}")),
        ("digits in capitals", format!("0x{}", bare.to_uppercase())),
        ("a digit that is not hex", format!("0x{}g", &bare[..63])),
        ("two prefixes", format!("0x{id}")),
    ] {
        let f = app::badgex::chain_for(&pool, &typed).expect_err(form);
        assert!(f.said().starts_with("CONTENT_SHAPE"), "{form}: {} {}", f.said(), f.tail());
        assert_eq!(f.tail(), typed, "{form}: the id as given");
        let f = app::badgex::code_for(&pool, &typed).expect_err(form);
        assert!(f.said().starts_with("CONTENT_SHAPE"), "{form}: the copied code says the same");
    }
    assert_eq!(app::badgex::chain_for(&pool, &format!("  {id}\n")).map(|c| c.len()).ok(), Some(1), "white space around a well-formed id is taken");
    assert!(app::badgex::code_for(&pool, &id).is_ok());
    let absent = app::badgex::chain_for(&pool, &format!("0x{}", "ab".repeat(32))).expect_err("absent");
    assert!(absent.said().starts_with("CHAIN_UNREACHED"), "a well-formed id not in the pool: {}", absent.said());
}

/// Text or a file written as a grant code is a code whatever the case of its prefix (the kit notes a prefix
/// not written exactly); it is never read as a path or an entry.
#[test]
fn a_code_with_its_prefix_in_another_case_is_a_code_said_by_the_kit() {
    let code_of = |prefix: &str| format!("{prefix}AAAA");
    for p in ["ZIKARON-GRANT:", "Zikaron-Grant:"] {
        let f = app::payloadx::take_full(&code_of(p)).err().expect("refused");
        assert!(f.said().starts_with("PAYLOAD_REFUSED") && f.tail().starts_with("E_BADGE_PREFIX"), "{p}: {} {}", f.said(), f.tail());
    }
    let dir = std::env::temp_dir().join(format!("zk-test-prefix-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join("code.txt");
    std::fs::write(&file, format!("{}\n", code_of("ZIKARON-GRANT:"))).expect("file");
    let f = app::payloadx::take_full(&file.display().to_string()).err().expect("refused");
    assert!(f.said().starts_with("PAYLOAD_REFUSED") && f.tail().starts_with("E_BADGE_PREFIX"), "{} {}", f.said(), f.tail());
    let lower = app::payloadx::take_full(&code_of("zikaron-grant:")).err().expect("refused");
    assert!(lower.said().starts_with("PAYLOAD_REFUSED") && !lower.tail().starts_with("E_BADGE_PREFIX"), "the exact prefix: the kit reads on");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A malformed delete or annotation subject is refused as a shape error with the ledger unchanged; a
/// well-formed id not in this ledger is reported absent.
#[test]
fn a_subject_written_another_way_is_its_shape_not_its_absence() {
    if alone("a_subject_written_another_way_is_its_shape_not_its_absence") {
        return;
    }
    vault_open();
    use app::action::{apply, Action, Applied};
    let base = std::env::temp_dir().join(format!("zk-subject-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    assert!(matches!(apply(&mut shell, Action::MakeAnchorKey), Applied::AnchorKey(_)));
    assert!(matches!(apply(&mut shell, Action::OpenHome { root: base.join("home").display().to_string() }), Applied::Homed { .. }));
    assert!(matches!(apply(&mut shell, Action::Genesis { statement: "subjects".into() }), Applied::Genesised { .. }));
    let file = base.join("work.txt");
    std::fs::write(&file, b"work").expect("a file");
    assert!(matches!(apply(&mut shell, Action::RecordWork { note_md: "w".into(), files: vec![file.display().to_string()], for_: None }), Applied::Started(_)));
    let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while shell.tasks.in_flight(app::task::Kind::Record) && std::time::Instant::now() < until {
        shell.drain();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    shell.drain();
    let pile = shell.home.as_ref().and_then(|h| h.ledger().ok()).and_then(|l| l.pile().ok()).map(|p| p.items).unwrap_or_default();
    let work = pile.iter().filter_map(|b| zikaron::entry::check(b).ok()).find(|e| e.kind == zikaron::tokens::EntryType::History).map(|e| e.id_hex()).expect("a recorded work");
    let id = format!("0x{}", work.trim_start_matches("0x"));
    let bare = id.trim_start_matches("0x").to_string();
    let entries = |shell: &app::shell::Shell| shell.home.as_ref().and_then(|h| h.ledger().ok()).and_then(|l| l.pile().ok()).map(|p| p.items.len()).unwrap_or(0);
    let before = entries(&shell);
    let absent = format!("0x{}", "00".repeat(32));
    let forms = [
        (format!("0x{}", bare.to_uppercase()), "CONTENT_SHAPE"),
        (bare.clone(), "CONTENT_SHAPE"),
        (format!("0X{bare}"), "CONTENT_SHAPE"),
        (format!("0x0x{bare}"), "CONTENT_SHAPE"),
        (format!("0x{}", &bare[1..]), "CONTENT_SHAPE"),
        (absent.clone(), "SUBJECT_MISSING"),
    ];
    for (subject, want) in &forms {
        for (verb, a) in [("retract", Action::Retract { subject: subject.clone(), note_md: "gone".into() }), ("annotate", Action::Annotate { subject: subject.clone(), note_md: "note".into() })] {
            match apply(&mut shell, a) {
                Applied::Trouble(f) => assert!(f.said().starts_with(want), "{verb} {subject}: {}", f.said()),
                other => panic!("{verb} {subject}: {other:?}"),
            }
        }
    }
    assert_eq!(entries(&shell), before, "nothing written");
    assert!(!matches!(apply(&mut shell, Action::Annotate { subject: id.clone(), note_md: "note".into() }), Applied::Trouble(_)), "the id as written: annotated");
    drop(shell);
    let _ = std::fs::remove_dir_all(&base);
}

/// One writer per home within one process too: a second take of the same home is a reader; once the writer
/// is released, the next take writes.
#[test]
fn a_second_take_of_a_home_reads_and_the_next_after_release_writes() {
    // A writer's lock writes this machine's mark into the test's own machine directory, never the real one.
    vault_open();
    let dir = std::env::temp_dir().join(format!("zk-test-lock2-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = app::home::Home::open_or_create(&dir).expect("建家");
    let first = app::lock::take(&home).expect("取锁");
    let second = app::lock::take(&home).expect("再取");
    assert_eq!((first.mode(), second.mode()), (app::lock::Mode::Writer, app::lock::Mode::Reader));
    drop(second);
    drop(first);
    assert_eq!(app::lock::take(&home).expect("放手后再取").mode(), app::lock::Mode::Writer);
    let _ = std::fs::remove_dir_all(&dir);
}

/// "Hide entries deleted on this machine" is a per-home, display-only setting (off by default) read only by
/// the records and ledger lists.
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

/// The "delete" convention sits on law §6.9's open types: the core lists a `retraction` under
/// `UNKNOWN_TYPE`, this desk reads it as "deleted", and invalid forms read as invalid without refusing the ledger.
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

    // The invalid forms then land in the same ledger (repeated delete, deleting genesis, deleting another
    // ledger's entry, malformed); the core accepts them and the ledger is not refused.
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

/// The wizard's gates (`Progress::gate`): no passcode sends every step to the passcode step; no identity sends
/// later steps to the identity step.
#[test]
fn the_wizard_never_stands_past_the_identity_step_without_an_identity() {
    use app::nav::{Progress, Step};
    // The passcode comes first: creating an identity needs keys, which need an open vault, so the reverse
    // order would make first run impossible.
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
    // The window really calls it, before drawing the step.
    let text = read_src_file("window.rs").expect("读不出 window.rs");
    let at_gate = text.find("let step = pr.gate(want);").expect("向导弹层里要过 Progress::gate");
    let at_draw = text.find("full::cover(ctx, \"wizard\"").expect("the wizard's cover");
    assert!(at_gate < at_draw, "门要在画弹层之前过");
}

/// Only gas funding and backup can be postponed (ledger creation too, for the user seat), decided by one
/// table (`Step::deferrable`).
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
    // Order: identity, then network, then ledger creation, which comes before gas funding.
    assert_eq!(Step::ALL, [Step::Pin, Step::Key, Step::Network, Step::Genesis, Step::Gas, Step::Backup]);
    assert!(!Step::Network.deferrable(Role::Author) && !Step::Network.deferrable(Role::Grantee), "网络一步默认那一行已选中,不可后做");
}

// ═════════════════════ Vault details · One key, one seat ═════════════════════

/// Actions carrying a `pin` cell register in `Action::pin_asked`, and `apply` checks the passcode once for
/// them, after "locked means refuse".
#[test]
fn the_three_actions_that_use_a_key_ask_the_one_passcode_gate() {
    let text = read_src_file("action.rs").expect("读不出 action.rs");
    let code = code_only(&text);
    // Every member with a `pin` cell (read from the `Action` source now, not copied as a list).
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
    // The three exits pass the gate; the five passcode gate actions themselves do not (asking the action, not a
    // string).
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
    // Checked exactly once, after "locked means refuse": a locked vault still refuses per the closed table.
    assert_eq!(code.matches("a.pin_asked()").count(), 1, "过闸那一句只许一处");
    let run = code.find("a.pin_asked()").expect("执行那一句");
    let locked = code.find("a.needs_key() && !shell.unlocked()").expect("锁着即拒那一句");
    assert!(locked < run, "「锁着即拒」要摆在口令闸之前");
    // Export and delete stay in the key-needing table; showing words asks its own unlock question, so it does not.
    assert!(A::BackupKey { pin: String::new().into(), password: String::new().into(), again: String::new().into(), dir: String::new() }.needs_key());
    assert!(A::DeleteIdentity { id: String::new(), pin: String::new().into() }.needs_key());
    assert!(!A::RevealWords { pin: String::new().into() }.needs_key());
}

/// Files are written in one place only (`home::put_at`, also used by the key vault), which creates them
/// owner-only (`zikaron_os::owner_only`), so no file ends up readable by other accounts through umask.
#[test]
fn every_small_file_this_desk_lands_is_owner_only() {
    let home = code_only(&read_src_file("home.rs").expect("读不出 home.rs"));
    assert!(home.contains("zikaron_os::owner_only(&mut o)"), "落档那一句建临时档时即只本人可读写");
    assert!(home.contains("create_new(true)"), "临时名撞上了即具名拒,不去截断别人那一份");
    assert!(home.contains("open_owner_only(&tmp)") && home.contains("replace_lasting(&tmp, &p)"), "临时档建时即只本人,替换进位");
    // On this system, a file written through the one write path is owner-only on disk.
    let dir = std::env::temp_dir().join(format!("zk-owner-only-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    app::home::put_at(&dir, "probe.json", b"{}").expect("落得下");
    assert!(zikaron_os::is_owner_only(&dir.join("probe.json")).expect("读得回"), "落下的档只本人可读写");
    let _ = std::fs::remove_dir_all(&dir);
    // The vault writes no files itself: `rename` and `File::create` appear nowhere in keybox.rs.
    let keybox = code_only(&read_src_file("keybox.rs").expect("读不出 keybox.rs"));
    for own in ["std::fs::rename", "File::create"] {
        assert!(!keybox.contains(own), "keybox.rs 里又自己落档了:{own}");
    }
    assert!(keybox.contains("crate::home::put_at(&dir, name, &bytes)"), "库档(与摆在旁边的新库)该经落档那一处");
    assert!(keybox.contains("crate::home::rename_over(&from, &to)"), "新库生效那一下改名也经家那一处");
}

/// One key, one seat: two seats with the same address cannot be constructed (enforced by the closed table
/// [`app::identity::Keys`]).
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
        unread_network: None,
    };
    assert_eq!(one.address(Role::Author), None);
    assert_eq!(one.address(Role::Grantee), Some(a));
    assert_eq!(one.home(Role::Author), None);
    assert_eq!(one.account(Role::Author), None);
    assert_eq!(one.accounts().len(), 1, "只占一席即只占一槽");
    assert_eq!(one.seats(), vec![Role::Grantee]);
    assert_eq!(one.first_seat(), Role::Grantee, "落在它占着的那一席上");
    // Both seats: kind and slot shape follow from the key's shape, with no separate cell.
    let both = Row { keys: Keys::Both { author: a, grantee: g }, author_home: "/x/a".into(), ..one.clone() };
    assert_eq!(both.kind(), app::identity::Kind::Words);
    assert_eq!(one.kind(), app::identity::Kind::Existing);
    assert_eq!(both.slot(), app::identity::Slot::Own);
    // Canonical byte round trip: an empty seat's address and home cells are UNSEATED, and the row reads back
    // the same.
    let reg = Registry { current: Some((one.id.clone(), Role::Grantee)), rows: vec![one.clone()], left: Vec::new() };
    assert_eq!(Registry::parse(&reg.to_bytes()).expect("读得回"), reg);
    // Rows written by older versions with both seats at one address and a home for each: read back, only the
    // recorder seat is occupied and the user seat is left empty.
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
    // Missing `label`, `created` and `backup.at` cells default without crashing.
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

/// "Backed up" is decided by reading the written file back (`BACKUP_NOT_LANDED` on mismatch), since a false
/// flag could lead a user to delete the identity and lose the key.
#[test]
fn a_backup_counts_only_when_the_file_is_really_on_disk() {
    let code = code_only(&read_src_file("action.rs").expect("读不出 action.rs"));
    // The read-back comes before "backed up" is recorded.
    let check = code.find("landed_check(&path, ks.address)?;").expect("落地之后该现读回来比一次");
    let done = code.find("Done::Keystore(crate::task::Keystore::BackedUp").expect("交出已备份那一句");
    assert!(check < done, "先读回来比过,再交出「已备份」");
    assert!(code.contains("Known::BackupNotLanded"), "对不上要具名拒");
    // Where the flag is recorded, the backup's location is recorded with it.
    let shell = code_only(&read_src_file("shell.rs").expect("读不出 shell.rs"));
    assert!(shell.contains("crate::identity::mark(reg, id, false, true, Some(path))"), "旗与落处一起记");
    // The reading function only reads, changing no cell.
    let identity = code_only(&read_src_file("identity.rs").expect("读不出 identity.rs"));
    let at = identity.find("pub fn backup_seen").expect("读数口");
    let body = &identity[at..identity[at..].find("\n}\n").map(|e| at + e).unwrap_or(identity.len())];
    for writes in ["write(", "std::fs::write", "remove_file", "mark("] {
        assert!(!body.contains(writes), "读数口改了盘上的东西:{writes}");
    }
    // The window asks it rather than checking file existence itself, and never in the frame: the identity card
    // reads the value saved when the register changed (`Shell::seat_identities`), the delete card the value saved
    // when the sheet opened.
    let window = code_only(&read_src_file("window.rs").expect("读不出 window.rs"));
    assert_eq!(window.matches("crate::identity::backup_seen").count(), 1, "脸上只许在开层那一刻问一处");
    assert!(window.contains("self.shell.backup_seen"), "身份卡读换手那一刻存下的那一格");
    assert_eq!(shell.matches("crate::identity::backup_seen").count(), 1, "换手那一处问一次");
}

/// Deleting an identity checks the ledgers in every seat home it occupies (from `Row::seats`, seat by seat),
/// not only the recorder's: otherwise a user-seat ledger or handover could be orphaned.
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

/// Which seat may sign which domain is decided only by `sign::seat_may`; other uses are refused as
/// `SEAT_DOMAIN`.
#[test]
fn which_seat_signs_which_domain_is_said_in_exactly_one_place() {
    use app::roles::Role;
    use app::sign::{Face, Use};
    // The closed set of three is computed from the two faces, so an extra domain adds a member here at once.
    assert_eq!(Use::all().len(), Face::ALL.len() + 1);
    // Six cells, checked one by one: this table owns the rule.
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
    // A use available to only one seat names which seat to switch to (the refusal needs it).
    assert_eq!(app::sign::seat_for(Use::Sign(Face::Adoption)), Some(Role::Author));
    assert_eq!(app::sign::seat_for(Use::Sign(Face::Entry)), None);
    // Every place fetching the signing key names a use: four places, each with a `sign::Use`.
    let code = code_only(&read_src_file("action.rs").expect("读不出 action.rs"));
    let calls: Vec<&str> = code.match_indices("= signing_key(shell").map(|(i, _)| &code[i..(i + 80).min(code.len())]).collect();
    // The fifth is the resend of a stuck batch: the anchoring key, named as every send names it.
    assert_eq!(calls.len(), 5, "取签名钥那一口现数 {} 处", calls.len());
    for one in &calls {
        assert!(one.contains("crate::sign::Use::"), "有一处取签名钥没有具名递用处:{one}");
    }
    // Decided in one place: `seat_may` is asked once in the action module, and no other action code builds its
    // own seat check.
    assert_eq!(code.matches("crate::sign::seat_may(").count(), 1, "席位 × 域那一问只许在取钥那一处问");
    assert!(code.contains("Known::SeatDomain"), "拒要具名 SEAT_DOMAIN");
    for wrong in ["shell.settings.role == crate::roles::Role::Grantee &&", "role != crate::roles::Role::Author {"] {
        assert!(!code.contains(wrong), "动作段里又自己拼了一次席位判:{wrong}");
    }
    // The window side computes from the table, with no copy of the list.
    let window = code_only(&read_src_file("window.rs").expect("读不出 window.rs"));
    assert!(window.contains("crate::sign::seat_may(seat, *u)"), "脸上那一行该照闭表现算");
}

/// Opening a home is decided in one place, so an author returning to their own home is never silently judged
/// a reader.
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
    // Both home-opening paths go through here: the one the user chose (`open_home`) and entering an identity's
    // seat (`enter`).
    for caller in ["fn open_home(", "fn enter("] {
        let a = code.find(caller).unwrap_or_else(|| panic!("找不到 {caller}"));
        let b = &code[a..code[a..].find("\n}\n").map(|e| a + e).unwrap_or(code.len())];
        assert!(b.contains("open_home_at("), "{caller} 该走开家那一处");
    }
}

/// Key files and whole-machine backups are owner-only, while files handed to the counterpart stay readable:
/// each caller names [`Readers`] per item.
#[test]
fn the_exported_key_file_is_for_its_owner_only_and_the_rest_are_not() {
    let code = code_only(&read_src_file("action.rs").expect("读不出 action.rs"));
    // The key file path names `Owner`.
    assert!(
        code.contains("land_bytes_for(zikaron_glue::landing::Readers::Owner, &path, &ks.json)"),
        "导出密钥文件那一路该具名 Readers::Owner"
    );
    // The backup's numbered landing names `Owner` too (a backup is the whole machine, sealed, for its owner).
    let home = code_only(&read_src_file("home.rs").expect("读不出 home.rs"));
    let numbered = home.split("pub fn land_numbered(").nth(1).and_then(|b| b.split("\npub fn ").next()).unwrap_or_default();
    assert!(numbered.contains("land_bytes_for(zikaron_glue::landing::Readers::Owner, &p, bytes)"), "备份起名落档那一处该具名 Readers::Owner");
    // No blanket rule: exactly those two places write with `Owner`.
    let mut owner = 0usize;
    for (_, text) in shipped() {
        owner += code_only(&text).matches("Readers::Owner").count();
    }
    assert_eq!(owner, 2, "具名 Owner 的落档现数 {owner} 处(只该是密钥文件与整机备份两路)");
    // The permission is set in one statement and the temporary file has it from creation (moving into place
    // uses a hard link, keeping the same inode's permission).
    let landing = std::fs::read_to_string(
        src().parent().expect("crates/app/src 的上一级").parent().expect("crates").join("zikaron-glue").join("src").join("landing.rs"),
    )
    .expect("读不出 landing.rs");
    assert!(landing.contains("zikaron_os::owner_only(&mut o)"), "只给本人那一路建临时地时即只本人可读写");
    assert_eq!(landing.matches("fn create_for(").count(), 1, "建临时地那一句只许一处");
    assert!(landing.contains("fn land_bytes_for("), "落档那一处该收「谁读得到」那一格");
}

// ═════════════════════ Vault lock, anchored record, landing place ═════════════════════

/// The vault file is read, changed and written under one lock, so concurrent processes cannot multiply the
/// passcode attempt limit; `write_book` requires the lock by type.
#[test]
fn the_key_store_is_written_only_with_its_lock_in_hand() {
    let code = code_only(&read_src_file("keybox.rs").expect("读不出 keybox.rs"));
    // The writer takes a reference to the lock, and the lock is taken in one place only.
    assert!(code.contains("fn write_book(_held: &Held, b: &Book)"), "写库那一处该收手里那一把锁");
    assert_eq!(code.matches("fn lock_book()").count(), 1, "取库锁那一句只许一处");
    assert!(code.contains("crate::lock::grab_waiting("), "库档那一把要等得起(拿不到即等着)");
    // Every vault writer has taken the lock earlier in the same function: count the "write" and "take lock" places.
    let writes = code.matches("write_book(&held").count() + code.matches("write_book(held,").count();
    assert_eq!(code.matches("write_book(").count(), writes + 1, "有一处写库没有把手里那一把递进去(+1 是定义那一处)");
    // Every exit that changes the vault starts by taking the lock (lock-taking places equal the number of exits).
    // Sixteen exits: set passcode, unlock, reseal at the lower bound, change passcode, recover, settle the
    // primary of an older vault, upgrade an older vault's names, record recovery seal, remove recovery seal,
    // place slot, remove slot, remove every slot, reset empty vault, stage a new vault, commit it, drop a staged one.
    assert_eq!(code.matches("lock_book()?").count(), 16, "要改库的那几口各拿一次锁");
    // The vault lock and the home's writer lock are separate: the vault lock lives in the machine directory,
    // named in one place by `places`.
    assert!(code.contains("crate::places::keybox_lock_file()"), "锁座那一份档的名由 places 一处给");
    let places = code_only(&read_src_file("places.rs").expect("读不出 places.rs"));
    assert!(places.contains("fn keybox_lock_file()"), "锁座那一份档的名住 places");
    assert!(!code.contains("crate::lock::take("), "库那一把不许借家的写者锁那一口");
    // The waiting and non-waiting exits each live in one place, with their words in lock.rs.
    let lock = code_only(&read_src_file("lock.rs").expect("读不出 lock.rs"));
    assert_eq!(lock.matches("pub fn grab(").count(), 1);
    assert_eq!(lock.matches("pub fn grab_waiting(").count(), 1);
}

/// "Anchored" is recorded in the queue file itself, so it survives restarts and an entry is never anchored
/// (and paid for) twice.
#[test]
fn the_queue_remembers_what_it_anchored() {
    use app::queue::{Pushed, Queue};
    let mut q = Queue::default();
    let a = format!("0x{}", "11".repeat(32));
    let b = format!("0x{}", "22".repeat(32));
    assert_eq!(q.push(&a, 1), Pushed::Queued);
    assert_eq!(q.push(&a, 2), Pushed::InQueue, "同一枚只排一次");
    assert_eq!(q.push(&b, 3), Pushed::Queued);
    // On the receipt path, removal and recording happen together, so no moment has it removed but unrecorded.
    assert_eq!(q.anchored_out(std::slice::from_ref(&a)), 1);
    assert!(q.anchored_here(&a));
    assert!(!q.anchored_here(&b));
    // The remove-only exit (used by retraction) records nothing: removed from the queue, not "anchored".
    let mut r = q.clone();
    assert_eq!(r.drop_ids(std::slice::from_ref(&b)), 1);
    assert!(!r.anchored_here(&b), "撤回离队不记「锚成过」");
    assert_eq!(q.push(&a, 4), Pushed::Anchored, "锚成过的一枚再也排不进来");
    assert_eq!(q.len(), 1, "另一枚照旧在队里");
    // There is one queueing entry point, and each of its three outcomes has a sentence (closed table).
    let code = code_only(&read_src_file("queue.rs").expect("读不出 queue.rs"));
    assert_eq!(code.matches("pub fn push(").count(), 1, "入队只此一处");
    assert_eq!(code.matches("pub fn drop_ids(").count(), 1, "只出队的那一口只此一处");
    assert_eq!(code.matches("pub fn anchored_out(").count(), 1, "出队并记事实的那一口只此一处");
    let at = code.find("pub fn anchored_out(").expect("收据那一路的口");
    let body = &code[at..code[at..].find("\n    }\n").map(|e| at + e).unwrap_or(code.len())];
    assert!(body.contains("self.anchored.push("), "收据说成了那一路出队时记下那一枚");
    // The cell is present in the canonical bytes; files written by older versions lack it and read it as empty,
    // not an error.
    let acted = code_only(&read_src_file("action.rs").expect("读不出 action.rs"));
    assert!(acted.contains("crate::queue::Pushed::Anchored =>"), "落完顺手排那一处要照形出话");
    assert!(acted.contains("shell.queue.anchored_here(&id)"), "页上手排那一处也问同一格");
}

/// Destinations are chosen in one place (`home::choose`), which also says why they differ from the choice
/// ([`Why`]).
#[test]
fn where_a_thing_lands_is_judged_in_one_place_and_says_why() {
    use app::home::{choose, Kind, Why, HOME_STEM};
    let base = std::env::temp_dir().join(format!("zk-landing-{}", std::process::id()));
    let empty = base.join("empty");
    let full = base.join("full");
    std::fs::create_dir_all(&empty).expect("铺一处空的");
    std::fs::create_dir_all(full.join("something")).expect("铺一处不空的");
    // Moving the home: an empty folder is used; a non-empty one gets a new name, and says why.
    let a = choose(&Kind::Home, &empty);
    assert_eq!((a.at.clone(), a.why), (empty.clone(), Why::AsPicked));
    let b = choose(&Kind::Home, &full);
    assert_eq!((b.at.clone(), b.why), (full.join(HOME_STEM), Why::FolderNotEmpty));
    assert!(b.free(), "新起的名此刻该空着");
    assert!(b.is_new_name());
    // Exporting a file: the first name is used when free; when taken, it is numbered and says why.
    let d1 = choose(&Kind::File { stem: "snap".into(), ext: "json".into() }, &empty);
    assert_eq!(d1.why, Why::AsPicked);
    std::fs::write(&d1.at, b"x").expect("占住那一个名");
    let d2 = choose(&Kind::File { stem: "snap".into(), ext: "json".into() }, &empty);
    assert_eq!((d2.at.clone(), d2.why), (empty.join("snap-2.json"), Why::NameTaken));
    // Exporting a bundle: a file the user names is used.
    let named = base.join("pick-me.zip");
    let e = choose(&Kind::Bundle { stem: "kit".into() }, &named);
    assert_eq!((e.at.clone(), e.why), (named, Why::AsPicked));
    let _ = std::fs::remove_dir_all(&base);
    // No separate destination functions exist; destinations are chosen only through `choose`.
    for (name, text) in shipped() {
        let code = code_only(&text);
        for gone in ["kit_target(", "migrate_target(", "file_target("] {
            assert!(!code.contains(gone), "{name} 里还留着落处那一族的旧成员:{gone}");
        }
    }
    let home = code_only(&read_src_file("home.rs").expect("读不出 home.rs"));
    assert_eq!(home.matches("pub fn choose(").count(), 1, "落处那一问只许一处判");
    assert_eq!(home.matches("fn numbered(").count(), 1, "往后编号那一句只许一处");
    // The window's sentence comes from `Why`, with nothing assembled on the window side.
    let window = code_only(&read_src_file("window.rs").expect("读不出 window.rs"));
    assert!(window.contains(".why.say()"), "搬家那一处要把「为什么」说出来");
    assert_eq!(window.matches("crate::home::choose(").count(), 4, "落处那几处调用各问同一句");
}

/// The identity and delete cards, import cells and move row read no disk in the frame (reading whole ledgers
/// every frame once froze the delete card).
#[test]
fn the_frame_body_asks_no_question_that_needs_the_disk() {
    let window = code_only(&read_src_file("window.rs").expect("读不出 window.rs"));
    // Each of the four is scanned: how often the name appears in the window, and where it should be.
    assert!(!window.contains("crate::identity::backup_seen("), "备份档那一笔不许在帧里读(壳上那一格由动作层摆)");
    // The ledger reading belongs to the action layer (`action::seats_with_entries`), asked once when the sheet
    // opens; the window opens no ledger itself.
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
    // The shell field is set by the same function that sets the identity table (one owner).
    let shell = code_only(&read_src_file("shell.rs").expect("读不出 shell.rs"));
    assert_eq!(shell.matches("pub fn seat_identities(").count(), 1, "摆身份表那一口只许一处");
    assert!(shell.contains("self.backup_seen = reg"), "那一笔读数与身份表同一趟换");
    // That field follows the identity table, not the ledger source: when the home changes it stays with
    // `identities` (in `source_changed`'s closed table it is in the group that does not follow the ledger source).
    assert!(shell.contains("backup_seen: _,"), "备份档那一笔随身份表走,不随账的来源走");
    // The identity table is set through one function only, so every writer goes through `seat_identities`;
    // otherwise the derived field could keep a stale answer from before the vault was locked.
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

/// "Which row and seat are current" is answered only by [`identity::now_row`] and
/// [`register::now_row_listed`] (the latter for signing, seat switching and home opening).
#[test]
fn which_identity_is_current_is_asked_in_two_named_places() {
    let identity = code_only(&read_src_file("identity.rs").expect("读不出 identity.rs"));
    let register = code_only(&read_src_file("register.rs").expect("读不出 register.rs"));
    assert_eq!(identity.matches("pub fn now_row(").count(), 1, "含账名底那一枚那一口只许一处");
    assert_eq!(register.matches("pub fn now_row_listed(").count(), 1, "只认登记表那一口只许一处");
    assert_eq!(identity.matches("view.now()").count(), 1, "那一读只在 now_row 里面");
    assert_eq!(register.matches("read()?.and_then(|r| r.now()").count(), 1, "那一读只在 now_row_listed 里面");
    // The backup and word display paths ask the same function.
    let action = code_only(&read_src_file("action.rs").expect("读不出 action.rs"));
    for (what, at) in [("显词", "fn reveal_words"), ("备份", "fn backup_key")] {
        let i = action.find(at).unwrap_or_else(|| panic!("{what}那一段"));
        let body = &action[i..action[i..].find("\n}\n").map(|e| i + e).unwrap_or(action.len())];
        assert!(body.contains("crate::identity::now_row("), "{what}那一路要问那一口");
    }
    // Nothing else takes its own copy of the register to ask "who is current" (the window and shell read the
    // copy in hand, without disk).
    for (name, text) in shipped() {
        if name == "identity.rs" || name == "register.rs" || name == "window.rs" || name == "shell.rs" {
            continue;
        }
        let code = code_only(&text);
        assert!(!code.contains(".now()"), "{name} 绕过了那两口,自己问了一次「谁是当前」");
    }
}

// ═════════════════════ Removal records its reason ═════════════════════

/// Only a receipt records "anchored" (`Queue::anchored_out`); other removals use `Queue::drop_ids`, and
/// re-queueing a deleted entry states the ledger fact, never a false chain fact.
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
    // The receipt exit is called only by `settle`.
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
    // Re-queueing a deleted entry: refused by name, stating the ledger fact.
    let action = code_only(&read_src_file("action.rs").expect("读不出 action.rs"));
    let at = action.find("fn queue_entry(").expect("入队那一扇门");
    let body = &action[at..action[at..].find("\n}\n").map(|e| at + e).unwrap_or(action.len())];
    assert!(body.contains("crate::retractx::read(&rows).is_deleted(&id)"), "被删了的那一条再入队要现读账问一次");
    assert!(body.contains("crate::lang::Key::Tail230"), "拒那一句说「此存证已删除」");
}

// ═════════════════════ Empty seats handled in two places ═════════════════════

/// Startup lands on a seat by `Row::first_seat` instead of stopping on an empty one, and seat emptiness is
/// decided only by `Shell::seat_unseated`.
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
    // Whether this seat is empty is decided once on the shell, with no condition written in the window.
    let shell = code_only(&read_src_file("shell.rs").expect("读不出 shell.rs"));
    assert_eq!(shell.matches("pub fn seat_unseated(").count(), 1, "一处判");
    assert!(!window.contains("r.address(seat).is_none()"), "窗子里不再自写「这一席空着」");
    assert!(window.contains("let unseated = self.shell.seat_unseated();"), "身份卡问那一口");
    assert!(window.contains("let seat_ok = !self.shell.seat_unseated();"), "数据卡问同一口");
    for key in ["t(Key::SetChangeHome), Role::Secondary, seat_ok)", "t(Key::DoMeasure), Role::Secondary, seat_ok, crate::task::Kind::Archive)"] {
        assert!(window.contains(key), "空席上这一键不可按:{key}");
    }
    // The wizard still requires a key, a passcode and (for an author) a genesis.
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

/// Secret cells (passcodes, passwords, words, private keys) use the secret type: zeroed on drop, masked in
/// debug output, never moved.
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
    // The key file a primary import writes carries its password twice, each a secret.
    let keyfile = body_of(&action, "pub struct KeyFileOut {");
    assert!(keyfile.contains("pub password: crate::secret::Secret,") && keyfile.contains("pub again: crate::secret::Secret,"), "导入时那一份密钥文件的密码也是秘密型");
    // Words shown to the user are held as secrets too: the shell's field after the passcode, the fresh words'
    // display copy, and the masked display that paints them.
    let shell_src = code_only(&read_src_file("shell.rs").expect("读不出 shell.rs"));
    assert!(shell_src.contains("pub words: Option<Vec<crate::secret::Secret>>,"), "过口令后显示的十二词是秘密型");
    let identity_src = code_only(&read_src_file("identity.rs").expect("读不出 identity.rs"));
    assert!(identity_src.contains("pub fn words(&self) -> Vec<crate::secret::Secret> {"), "新生成的词的显示副本是秘密型");
    let pin_src = std::fs::read_to_string(src().join("..").join("..").join("zikaron-ui").join("src").join("pin.rs")).expect("读不出 pin.rs");
    assert!(pin_src.contains("words: Option<&[crate::secret::Secret]>"), "遮住的显示收秘密型");
    // The widget library type's own properties: zeroing and masked debug output (`secret.rs`'s unit tests check
    // the bytes).
    let lib = std::fs::read_to_string(src().join("..").join("..").join("zikaron-ui").join("src").join("secret.rs")).expect("读不出 secret.rs");
    assert!(lib.contains("impl Drop for Secret") && lib.contains("impl std::fmt::Debug for Secret"), "秘密型要自己抹零、自己遮调试输出");
}

/// The twelve word cells are always masked by the widget itself, the gate never decodes them in the frame,
/// and "back" clears all twelve.
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

/// Deleting an identity: the primary one is refused (another must be made primary first); a secondary one
/// takes along any recovery seal an older vault still holds for it; the comment matches the code.
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

/// The post-landing test hook is set only by test code; in the shipped build it is always empty.
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

/// Node refusals are dispatched in one place: anchoring failures are not collapsed into `SEND_FAILED`, and
/// node refusal codes come only from `chainx::said_fault` (the pre-send balance gate has its own place).
#[test]
fn what_the_node_said_is_dispatched_in_one_place() {
    let sign = code_only(&read_src_file("sign.rs").expect("读不出 sign.rs"));
    assert!(!sign.contains("Known::SendFailed"), "发锚那一路不许再把节点的话压成 SEND_FAILED");
    assert!(sign.contains("crate::chainx::said_fault(url.for_transport(), &t)"), "发锚那一路按节点说的话分派");
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
    // The node's answer is read in one place for the app and the command line (`rpc::ask_node`,
    // `rpc::read_answer`), and the refusal table is the shared one (`zikaron_anchor::said`), not a copy here.
    let chainx = code_only(&read_src_file("chainx.rs").expect("读不出 chainx.rs"));
    assert!(chainx.contains("rpc::ask_node("), "应用那一扇 https 端点经共用那一处问与读");
    assert!(!chainx.contains("got.body") && !chainx.contains("member(\"result\")"), "应用不另读一份应答");
    assert!(!chainx.contains("const MARKS"), "拒因表只一份,在共用层");
    // "Sent but failed on chain" is kept only for a receipt status other than 1.
    let window = code_only(&read_src_file("window.rs").expect("读不出 window.rs"));
    assert_eq!(window.matches("Known::SendFailed").count(), 1, "SEND_FAILED 只在收据失败那一处说");
    assert!(app::action::receipt_failed(&app::action::receipt_state(0, 9)));
    assert!(!app::action::receipt_failed(&app::action::receipt_state(1, 9)));
    assert!(!app::action::receipt_failed("not yet in a block"));
}

/// Node answers map to fault members through one table, and a refusal is about the call itself exactly when
/// its member is outside the network family.
#[test]
fn what_the_node_said_reads_by_one_table_for_the_app_and_the_command_line() {
    use app::chainx::{said_fault, Refusal};
    use app::fault::Known;
    use zikaron_anchor::rpc::{self, Trouble};
    let node = |m: &str| Trouble::Node(format!("{{\"code\":-32000,\"message\":\"{m}\"}}"));
    for (said, r, k) in [
        ("insufficient funds", Refusal::Funds, Known::InsufficientFunds),
        ("nonce too low", Refusal::NonceUsed, Known::NonceUsed),
        ("already known", Refusal::Pending, Known::AlreadyPending),
        ("transaction underpriced", Refusal::Underpriced, Known::Underpriced),
        ("intrinsic gas too low", Refusal::GasTooLow, Known::GasTooLow),
        ("execution reverted", Refusal::Reverted, Known::ContractRefused),
        ("too many requests", Refusal::RateLimited, Known::RateLimited),
        ("method not found", Refusal::NoMethod, Known::MethodMissing),
        ("unauthorized", Refusal::Auth, Known::NodeAuth),
        ("wrong chain", Refusal::WrongChain, Known::WrongChain),
        ("something else", Refusal::Coded(String::new()), Known::NodeRefused),
    ] {
        let t = node(said);
        assert_eq!(said_fault("u", &t).which(), Some(k), "{said}");
        // A coded node refusal the table does not name is about the call (an estimate refused this way would
        // revert) yet still reported as the node's refusal and counted in the network family for the status line
        // and a send's next step: the one member where the two readings differ.
        if matches!(r, Refusal::Coded(_)) {
            assert!(r.about_the_call() && Known::NETWORK.contains(&k), "{said}");
        } else {
            assert_eq!(r.about_the_call(), !Known::NETWORK.contains(&k), "{said}: the one table and the network family agree");
        }
    }
    let url = "https://n.example";
    assert_eq!(said_fault(url, &rpc::status(url, 429)).which(), Some(Known::RateLimited));
    assert_eq!(said_fault(url, &rpc::status(url, 401)).which(), Some(Known::NodeAuth));
    assert_eq!(said_fault(url, &rpc::status(url, 403)).which(), Some(Known::NodeAuth));
    let refused = said_fault(url, &rpc::status(url, 502));
    assert_eq!(refused.which(), Some(Known::NodeRefused));
    assert!(refused.tail().contains("502"), "the status is in the refusal: {}", refused.tail());
    assert_eq!(said_fault(url, &rpc::shapeless(url)).which(), Some(Known::ChainShape));
    assert_eq!(said_fault(url, &Trouble::Transport(rpc::NOT_JSON.into())).which(), Some(Known::AnswerNotJson));
    // What sending does next, by the same members: rate limited retries the same node, the rest of the network
    // family moves on, a refusal about the call stops.
    use app::chainx::{next_after, Layer, Next};
    assert_eq!(next_after(&said_fault(url, &rpc::status(url, 429))), Next::Retry);
    assert_eq!(next_after(&said_fault(url, &rpc::status(url, 502))), Next::NextEndpoint);
    assert_eq!(next_after(&said_fault(url, &rpc::shapeless(url))), Next::NextEndpoint);
    assert_eq!(next_after(&said_fault(url, &node("insufficient funds"))), Next::Stop);
    // The four layer codes lead their sentences and read back as their layers; nothing else is a layer.
    for l in Layer::ALL {
        assert_eq!(Layer::of(&format!("{} · words", l.code())), Some(l));
    }
    assert_eq!(Layer::of(&format!("{url} 答 HTTP 403")), None);
    // Every member here is one of the network family's fourteen, or a refusal about the call.
    assert_eq!(Known::NETWORK.len(), 14);
}

/// Passcodes allow letters and digits, case-sensitive; all-identical is refused for any passcode, and
/// sequences and dates are checked only for all-digit ones.
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

/// Every legal refusal token has a distinct plain sentence in both languages, and the evidence tail keeps
/// the token's name.
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

/// Masked cells do not open the input method: IME events are filtered while a secret cell has focus.
#[test]
fn a_masked_field_turns_the_input_method_off() {
    let input = code_only(&std::fs::read_to_string(src().join("../../zikaron-ui/src/input.rs")).expect("读不出 input.rs"));
    let body = body_of(&input, "fn secret_field(");
    assert!(body.contains("egui::Event::Ime(_)"), "握着焦点时输入法事件整枚不收");
    assert!(body.contains("o.ime = None"), "画完撤掉这一帧的输入法请求");
    assert_eq!(input.matches(".password(true)").count(), 1, "遮住的格只此一件");
}

/// Sending or waiting after queueing is decided by one setting ("auto anchor", off by default), and the main
/// button's label is chosen by one function.
#[test]
fn the_primary_key_reads_the_auto_anchor_setting_in_one_place() {
    assert!(!app::settings::Settings::default().auto_anchor, "自动上链默认关");
    let window = code_only(&read_src_file("window.rs").expect("读不出 window.rs"));
    let pick = body_of(&window, "fn anchor_key(");
    assert!(pick.contains("settings.auto_anchor") && pick.contains("Key::AnchorNowKey") && pick.contains("Key::AddToLedgerKey"));
    assert!(window.matches("self.anchor_key()").count() >= 4, "存证与签发两页的主键与确认卡都读它");
    assert!(!window.contains("anchor_queue_only"), "表单里那一枚内存态单选撤了");
}

/// The queue file's seven states read back from disk ("resent" with every transaction of its batch); files
/// written by older versions, without state cells or block tables, still read as "queued".
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
    q.mark(&[id(2)], Step::Submitted { tx: id(9), chain: 11155111, nonce: Some(7) });
    q.mark(&[id(3)], Step::Reverted { tx: id(8), chain: 11155111 });
    q.mark(&[id(4)], Step::Refused { said: "INSUFFICIENT_FUNDS".into() });
    q.push(&id(5), 5);
    q.included_out(&[id(5)], &id(7), 11155111, 16);
    // Resent: every transaction of the batch, first sent first.
    q.push(&id(6), 6);
    q.mark(&[id(6)], Step::Resent { txs: vec![id(10), id(11)], chain: 11155111, nonce: None });
    q.write(&h).expect("写");
    let back = Queue::read(&h).expect("读");
    assert_eq!(back.step_of(&id(1)), Some(&Step::Queued));
    assert_eq!(back.step_of(&id(2)), Some(&Step::Submitted { tx: id(9), chain: 11155111, nonce: Some(7) }));
    assert_eq!(back.step_of(&id(3)), Some(&Step::Reverted { tx: id(8), chain: 11155111 }));
    assert_eq!(back.step_of(&id(4)), Some(&Step::Refused { said: "INSUFFICIENT_FUNDS".into() }));
    assert_eq!(back.block_of(&id(5)).map(|b| b.block), Some(16));
    assert!(back.anchored_here(&id(5)) && !back.has(&id(5)));
    assert_eq!(back.step_of(&id(6)), Some(&Step::Resent { txs: vec![id(10), id(11)], chain: 11155111, nonce: None }));
    assert_eq!(back.submitted(), vec![(vec![id(9)], 11155111, vec![id(2)]), (vec![id(10), id(11)], 11155111, vec![id(6)])]);
    // In flight, submitted or resent: never picked for another batch, never counted as sendable.
    assert!(!back.take_ids(10).contains(&id(2)) && !back.take_ids(10).contains(&id(6)));
    assert_eq!(back.sendable(), 3);
    // A resent row with one transaction, or with more than the first plus three resends, is a shape error.
    for n in [1usize, 2 + app::queue::RESENDS_MAX] {
        let txs: Vec<String> = (0..n).map(|i| format!("\"{}\"", id(20 + i as u8))).collect();
        let bad = format!("{{\"anchored\":[],\"queued\":[{{\"at\":1,\"chain\":1,\"id\":\"{}\",\"state\":\"resent\",\"txs\":[{}]}}]}}", id(1), txs.join(","));
        app::local::put(&h.dir(app::home::Slot::Settings), app::queue::FILE, app::local::Doc::Queue, bad.as_bytes()).expect("写坏档");
        assert_eq!(Queue::read(&h).err().and_then(|f| f.which()), Some(app::fault::Known::QueueShape), "{n} transactions");
    }
    // An older file: only `queued` and `anchored`, with no state cell in rows.
    let old = format!("{{\"anchored\":[],\"queued\":[{{\"at\":1,\"id\":\"{}\"}}]}}", id(1));
    // An older version's shape, sealed as the queue file is now (the shape is under test, not the seal).
    app::local::put(&h.dir(app::home::Slot::Settings), app::queue::FILE, app::local::Doc::Queue, old.as_bytes()).expect("写旧档");
    let old = Queue::read(&h).expect("旧档读得回");
    assert_eq!(old.step_of(&id(1)), Some(&Step::Queued));
    assert!(old.blocks.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

// ═════════════════════ Landing and export · known deployments and new home basis ═════════════════════

/// The five landing failure forms each have plain words and a next step; the code stays `LANDING`, and the
/// tail carries only the refusal code and subject.
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

/// Kit name transliteration happens in one place, using the kit crate's `is_kit_path` as the rule.
#[test]
fn kit_names_are_judged_by_the_kit_law_in_one_place() {
    let kitx = code_only(&read_src_file("kitx.rs").expect("读不出 kitx.rs"));
    assert!(body_of(&kitx, "pub fn kit_segment(").contains("kitdir::is_kit_path"));
    assert!(body_of(&kitx, "pub fn attach(").contains("kit_rel("), "附件逐件走转写那一处");
    let window = code_only(&read_src_file("window.rs").expect("读不出 window.rs"));
    assert!(window.contains("crate::kitx::preview_names("), "附件列表预览与出包同一处转写");
}

/// The backup destination is assembled in one place: `ZIKARON-backup/<address>/<seat>`; empty and relative
/// paths are refused.
#[test]
fn the_backup_bundle_path_is_built_in_one_place() {
    use app::roles::Role;
    // An absolute folder on this system (the temporary directory is absolute everywhere).
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

/// The known deployments table is compiled in, and chain id and contract always come from it; a new home
/// takes its basis through one entry point only.
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
    // A network (a known row or one filled in by hand) enters a home through one entry point, called from two
    // places: a writer opening a home without a network takes its identity's, and the wizard's network step
    // fills the current home.
    assert_eq!(action.matches("adopt_network(shell, ").count(), 2, "two call sites use the one entry point");
    assert_eq!(action.matches("adopt_deployment(shell, d)").count(), 1, "a known row fills a home inside the one entry point only");
    let machine = code_only(&read_src_file("machine.rs").expect("读不出 machine.rs"));
    assert!(!machine.contains("11155111") && !machine.contains("11_155_111"), "机器级设置档不留链号的第二份说法");
}

// ───────────────────── What the verifier should receive ─────────────────────

/// Audit input is resolved level by level in a closed table of four fixed levels: the check page and the
/// vault re-check both fetch material through `supplyx::find`, and nothing else opens another path to a ledger.
#[test]
fn the_audit_input_is_resolved_level_by_level_in_one_place() {
    use app::supplyx::Level;
    assert_eq!(Level::ALL, [Level::Local, Level::Vault, Level::Kit, Level::Remote], "the levels are tried in exactly this order");
    // Level names are not pinned; only that each of the four levels has its own name.
    let mut names: Vec<&str> = Level::ALL.iter().map(|l| l.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), Level::ALL.len(), "四级各有各的名");
    assert!(Level::Local < Level::Vault && Level::Vault < Level::Kit && Level::Kit < Level::Remote);
    let action = code_only(&read_src_file("action.rs").expect("读不出 action.rs"));
    assert_eq!(action.matches("supplyx::find(").count(), 1, "动作这一层只有保管库复核自己调那一处;查验页经 checkx::run");
    let checkx = code_only(&read_src_file("checkx.rs").expect("读不出 checkx.rs"));
    assert_eq!(checkx.matches("supplyx::find(").count(), 1);
    // Ledger-fetching paths live only in `supplyx`: other modules may not open other homes via the register.
    for (name, text) in shipped() {
        // `firstrun.rs` opens this home's own ledger (first-run inventory), and `local.rs` is where every sealed
        // ledger on this machine is opened.
        if name == "supplyx.rs" || name == "firstrun.rs" || name == "local.rs" {
            continue;
        }
        assert!(!code_only(&text).contains("LedgerDir::open("), "{name} 自己去开别人的账本");
    }
}

/// Every grey light speaks from the result object: each missing thing has its own form, grey is kept apart
/// from red, an answered state never has a gap, and the six gaps' action sentences all differ.
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
    // An answered state (PASS/FAIL) has no gap.
    let window = code_only(&read_src_file("window.rs").expect("读不出 window.rs"));
    assert!(window.contains("crate::checkx::gap(&x,"), "脸上的缺口由 checkx 出,不在渲染层重算");
    let checkx = code_only(&read_src_file("checkx.rs").expect("读不出 checkx.rs"));
    assert!(checkx.contains("if state != State::Unknown.as_str()"), "只对未定那一态答");
}

/// Exclusivity is written once at signing and read-only afterwards: no action flips the flag, only issuing
/// writes it, and the older list in the settings file is read-only.
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
    // A three-state closed table: recorded at signing, the older list (no terms document), or none.
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

/// A grant file is an enumeration handed to the same kit verification: the container checks no hash itself;
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
    // The reading lives in glue, which the command line also calls; the app only maps its refusals.
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

/// Remote fetching uses the one TLS client, https only: there is no second fetch path, the publish address
/// refuses http and other schemes, and the test-only trust root function is never called.
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
    // The transport builds its TLS client configuration in one place.
    let net = code_only(&std::fs::read_to_string(src().join("../../zikaron-net/src/lib.rs")).expect("读不出 zikaron-net"));
    assert_eq!(net.matches("ClientConfig::builder").count(), 1, "TLS 配置只建一处");
    let settings = code_only(&read_src_file("settings.rs").expect("读不出 settings.rs"));
    assert!(settings.contains("fetchx::base_of(x)"), "设置档里读回来的发布地址照同一处认");
}

// ───────────────────── Widget library and pages ─────────────────────

/// Boolean state uses one component: the source has no `ui.checkbox`; exclusivity and auto anchor both use the
/// widget library's switch (`toggle`).
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

/// Blocks sit side by side only through the equal-cells component (`grid::tiles`): the page layer never calls
/// `ui.columns`, whose justified layout stretches letter spacing in wrapped strings.
#[test]
fn side_by_side_goes_through_the_one_grid() {
    for (name, text) in shipped() {
        assert!(!code_only(&text).contains("ui.columns("), "{name} 还直接调 ui.columns");
    }
    let grid = code_only(&std::fs::read_to_string(src().join("../../zikaron-ui/src/grid.rs")).expect("读不出等分格件"));
    assert!(grid.contains("egui::Layout::top_down(egui::Align::Min)"), "格里的布局不两端对齐");
    assert!(!grid.contains("top_down_justified"), "格里的布局不两端对齐");
}

/// The final-step (solid red) button appears only on confirmation sheets; guide buttons may appear anywhere.
#[test]
fn the_solid_red_key_is_only_on_a_confirm_card() {
    let text = read_src_file("window.rs").expect("读不出 window.rs");
    let lines: Vec<&str> = text.lines().collect();
    let mut at: Vec<(usize, String)> = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if l.contains("page::Pen::new()") {
            // Find the owning fn, searching upwards.
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
        "fn bump_sheet(",
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
        // A fetch found this home at odds with the fetched ledger: "fetch and replace".
        "fn conflict_sheet(",
    ];
    for (line, owner) in &at {
        assert!(sheets.iter().any(|s| owner.contains(s)), "window.rs:{line} takes the final-step token outside a confirmation sheet: {owner}");
    }
    for s in sheets {
        // The identity sheets hold two final steps (delete, set as primary); the backup sheets two (restore from
        // settings or at first run, replace on the locked card's confirm).
        let want = if s == "fn id_sheets(" || s == "fn bk_sheets(" { 2 } else { 1 };
        assert_eq!(at.iter().filter(|(_, o)| o.contains(s)).count(), want, "{s} holds exactly {want} final-step key(s)");
    }
    // In the widget library, final-step and guide buttons have separate tokens, and guide buttons have no
    // one-per-page limit.
    let page = code_only(&std::fs::read_to_string(src().join("../../zikaron-ui/src/page.rs")).expect("读不出 page.rs"));
    assert!(page.contains("pub struct Guide"), "引导键那一枚令牌在件库里");
    assert!(page.contains("pub fn commits_in"), "「确认卡之外零实心深红」那一条现算腿在件库里");
}

/// Embedded fonts by system: macOS and Windows embed only Latin and monospace faces (SIL OFL 1.1), other
/// systems embed all; embedded faces carry their licence, system faces their path.
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

/// The exit gate's closed table: every action says whether it lets this ledger's facts leave the machine
/// (`Action::exit`), one arm each and no catch-all, so a new action does not compile until classified.
#[test]
fn every_action_answers_whether_it_is_an_exit() {
    let code = code_only(&read_src_file("action/mod.rs").expect("读不出 action/mod.rs"));
    let at = code.find("pub fn exit(&self) -> Option<crate::exitgate::Exit>").expect("归类那一口在");
    let body = &code[at..];
    let end = body.find("\n    }\n").expect("那一口的尾");
    let body = &body[..end];
    assert!(!body.contains("_ =>"), "出口归类不许有通配臂");
    assert_eq!(body.matches("=> Some(Exit::").count(), 5, "出口恰五员");
    // Five actions, not five arms: an arm naming two actions would hide a sixth exit.
    let exiting: usize = body.split("=> Some(Exit::").take(5).map(|arm| arm.rsplit('\n').next().unwrap_or("").matches("Action::").count()).sum();
    assert_eq!(exiting, 5, "恰五个动作让事实离开本机,一臂一个");
    assert_eq!(app::exitgate::Exit::ALL.len(), 5);
}

/// Each of the five effects that let facts leave the machine takes the exit gate's `Pass`, which only the
/// gate makes after reading the chain, so the compiler rules out a bypass.
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
    // The pass is built only by the gate: its fields are private and its one struct literal is in `pass`.
    let gate = code_only(&read_src_file("exitgate.rs").expect("exitgate"));
    assert!(gate.contains("pub struct Pass {\n    reading: Reading,\n    root: PathBuf,\n}"), "令牌的成员须是私有的");
    assert_eq!(gate.matches("Pass { reading").count(), 1, "令牌只在闸里造一处");
    for (name, text) in shipped().into_iter().filter(|(n, _)| n != "exitgate.rs") {
        assert!(!code_only(&text).contains("Pass { reading"), "{name} 自造了令牌");
    }
    let q = code_only(&read_src_file("action/queue.rs").expect("queue"));
    assert!(q.find("funds_gate(&b, &secret, fees)").unwrap_or(usize::MAX) < q.find("crate::exitgate::pass(&ask)").unwrap_or(0), "发交易:余额那一判在出口闸之前");
}

/// The put-on-chain sheet closes when its broadcast lands, judged by the landing, never by the clock.
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
    // Wait for the worker to end (its outcome is sent before it ends), not for a fixed time.
    while !probe.shell().tasks.finished_in_flight(app::task::Kind::Anchor) {
        std::thread::yield_now();
    }
    let closed = (0..5).any(|_| !probe.frame(&ctx));
    assert!(closed, "the broadcast landed and the sheet stayed open");
}

#[test]
fn the_tail_check_is_recorded_as_asked_before_it_is_asked() {
    // The window's timer polls the tail check every frame. Taking it records the request for this ledger state
    // at once, so whatever answers (even a refusal before the check starts) is not asked and reported again next
    // frame; the ledger moving makes it due again.
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

/// The exports are one table (`kitsindex::EXPORTS`), and the two with a fixed place are where the path
/// builders put them.
#[test]
fn the_exports_land_where_the_one_table_says() {
    use app::kitsindex::EXPORTS;
    let m = std::path::Path::new("/machine");
    let mut names: Vec<&str> = EXPORTS.iter().map(|e| e.name).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), EXPORTS.len(), "each export once");
    let at = |name: &str| EXPORTS.iter().find(|e| e.name == name).copied().expect(name);
    let joined = |p: &[&str]| p.iter().fold(m.to_path_buf(), |a, x| a.join(x));
    let index = at("kitsIndex");
    assert_eq!((joined(index.place.expect("a place")), index.room), (app::kitsindex::path_in(m), false));
    let room = at("kitVerifications");
    let file = app::verifiedx::path_of(m, &format!("0x{}", "ab".repeat(32)));
    assert_eq!((Some(joined(room.place.expect("a place"))), room.room), (file.parent().map(|p| p.to_path_buf()), true), "the result file is directly in the room");
    for name in ["kit", "mirror", "machineBackup", "grantFile"] {
        assert!(at(name).place.is_none(), "{name}: where the person picks");
    }
}

/// A kit note's anchoring line (also read by the sibling app GALEED) is exactly its last non-empty line;
/// near misses are not it, and re-exporting never doubles the line.
#[test]
fn the_anchoring_line_is_the_last_non_empty_line_of_the_note() {
    use app::kitsindex::{note_with, split_note, AnchoredOn};
    let at = AnchoredOn { chain_id: 8453, from_block: 24_100_000, registry: format!("0x{}", "7a".repeat(20)) };
    let line = at.line();
    for (form, note, author) in [
        ("authorThenLine", format!("给判官的一包\n第二行\n{line}"), "给判官的一包\n第二行"),
        ("lineOnly", line.clone(), ""),
        ("aLineFeedAfter", format!("一包\n{line}\n"), "一包"),
        ("blankLinesAfter", format!("一包\n{line}\n\n  \n\t\n"), "一包"),
        ("carriageReturnsAfter", format!("一包\r\n{line}\r\n"), "一包"),
        ("authorKeepsItsOwnBreaks", format!("a\r\nb\n\nc\n{line}"), "a\r\nb\n\nc"),
    ] {
        assert_eq!(split_note(&note), (author.to_string(), Some(at.clone())), "{form}");
        assert_eq!(note_with(&note, Some(&at)), if author.is_empty() { line.clone() } else { format!("{author}\n{line}") }, "{form}: never doubled");
    }
    let near = [
        line.replace("eip155:8453", "eip155:08453"),
        line.replace(&"7a".repeat(20), &"7A".repeat(20)),
        line.replace(" \u{b7} registry", " registry"),
        format!("{line} "),
    ];
    for (form, note) in [
        ("textAfterTheLine", format!("{line}\nmore words")),
        ("empty", String::new()),
        ("whiteSpaceOnly", "  \n\t\n".to_string()),
        ("leadingZero", format!("一包\n{}", near[0])),
        ("capitalAddress", format!("一包\n{}", near[1])),
        ("separatorShort", format!("一包\n{}", near[2])),
        ("spaceAfter", format!("一包\n{}", near[3])),
    ] {
        assert_eq!(split_note(&note), (note.clone(), None), "{form}: not the line, the note is the author's text whole");
    }
}

/// The kit verification result file (`verifiedx`), as the sibling apps GALEED and ERAVON read it, has one
/// canonical shape and reads back as written.
#[test]
fn the_result_file_has_its_one_shape() {
    use app::verifiedx::{path_of, value_of, write, Found, FORM};
    use zikaron::json::Value;
    let manifest = b"manifest bytes".to_vec();
    let kit = app::verifyx::KitFacts { ok: true, entries: 2, files: 1, proofs: 0, kit_id: format!("0x{}", "ab".repeat(32)), verdict: String::new(), subject: String::new(), invalid: Vec::new() };
    let first = app::auditx::FirstAnchor { chain_id: 8453, block_number: 20, block_timestamp: 1_790_000_240, tx: format!("0x{}", "cd".repeat(32)), hash: format!("0x{}", "ef".repeat(32)), registry: Some(format!("0x{}", "7a".repeat(20))) };
    let row = |content: &str, first: Option<app::auditx::FirstAnchor>| app::verifyx::RecordRow {
        id: format!("0x{}", "01".repeat(32)),
        content: content.to_string(),
        name: None,
        original: app::verifyx::Original::Match,
        chain: app::verifyx::OnChain::NotAnchored,
        first,
    };
    let records = vec![row(&format!("0x{}", "11".repeat(32)), Some(first.clone())), row(&format!("0x{}", "22".repeat(32)), None)];
    let anchor = |hash: &str| Value::Obj(vec![
        ("blockNumber".into(), Value::Int(20)),
        ("chainId".into(), Value::Int(8453)),
        ("hash".into(), Value::Str(hash.to_string())),
        ("tx".into(), Value::Str(format!("0x{}", "cd".repeat(32)))),
    ]);
    let (h_noted, h_bare) = (format!("0x{}", "ef".repeat(32)), format!("0x{}", "fe".repeat(32)));
    let fragment = Value::Obj(vec![("anchors".into(), Value::Arr(vec![anchor(&h_noted), anchor(&h_bare)])), ("basis".into(), Value::Obj(vec![("chains".into(), Value::Arr(Vec::new()))]))]);
    let mut emitters = zikaron_anchor::scan::Emitters::new();
    let key = (8453u64, 20u64, [0xcdu8; 32], [0xefu8; 32]);
    emitters.entry(key).or_default().insert([0x7a; 20]);
    let missed_one = app::widex::Missed { chain_id: 10, registry: app::key::Address([0x3f; 20]), from_block: 0, name: String::new(), reading: app::widex::Reading::Down };
    let missed = vec![missed_one.clone(), app::widex::Missed { chain_id: 1, registry: app::key::Address([0x0d; 20]), from_block: 0, name: String::new(), reading: app::widex::Reading::Fingerprint }, missed_one];
    let held = value_of(&Found { manifest: &manifest, at: 1_790_726_400, kit: &kit, records: &records, fragment: Some(&fragment), emitters: &emitters, missed: &missed });
    let keys = |v: &Value| match v {
        Value::Obj(m) => m.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    assert_eq!(keys(&held), ["anchors", "at", "basis", "core", "form", "kit", "kitVerdict", "manifestSha256", "missed", "records"]);
    assert_eq!(held.member("form").and_then(|x| x.as_str()), Some(FORM));
    assert_eq!(held.member("kit").and_then(|x| x.as_str()), Some(kit.kit_id.as_str()));
    assert_eq!(held.member("manifestSha256").and_then(|x| x.as_str()), Some(zikaron::hexfmt::encode(&zikaron::cryptox::sha256(&manifest)).as_str()), "the sha256 of the manifest's bytes");
    assert_eq!(keys(held.member("kitVerdict").expect("a verdict")), ["verdict"], "no subject when it holds");
    let Some(Value::Arr(anchors)) = held.member("anchors") else { panic!("anchors") };
    assert_eq!(anchors[0].member("registry").and_then(|x| x.as_str()), Some(format!("0x{}", "7a".repeat(20)).as_str()));
    assert_eq!(anchors[1].member("registry"), Some(&Value::Null), "no registry noted: null");
    let Some(Value::Arr(rec)) = held.member("records") else { panic!("records") };
    assert_eq!(keys(&rec[0]), ["anchor", "content"]);
    assert_eq!(keys(rec[0].member("anchor").expect("an anchor")), ["blockNumber", "blockTimestamp", "chainId", "registry", "tx"]);
    assert_eq!(rec[1].member("anchor"), Some(&Value::Null));
    let Some(Value::Arr(m)) = held.member("missed") else { panic!("missed") };
    assert_eq!(m.iter().map(|x| x.member("chainId").cloned()).collect::<Vec<_>>(), [Some(Value::Int(1)), Some(Value::Int(10))], "once each, in order");
    assert_eq!(keys(&m[0]), ["chainId", "reading", "registry"]);
    assert_eq!(m[0].member("reading").and_then(|x| x.as_str()), Some("fingerprint"));
    // A kit that does not hold: its verdict and subject, nothing of the chain.
    let broken = app::verifyx::KitFacts { ok: false, verdict: "KIT_MANIFEST_MISMATCH".into(), subject: "files/a.bin".into(), ..kit.clone() };
    let refused = value_of(&Found { manifest: &manifest, at: 1, kit: &broken, records: &records, fragment: Some(&fragment), emitters: &emitters, missed: &missed });
    assert_eq!(keys(&refused), ["anchors", "at", "core", "form", "kitVerdict", "manifestSha256", "missed", "records"]);
    assert_eq!(keys(refused.member("kitVerdict").expect("a verdict")), ["subject", "verdict"]);
    for k in ["anchors", "missed", "records"] {
        assert_eq!(refused.member(k), Some(&Value::Arr(Vec::new())), "{k}");
    }
    // Where it is written, and that it reads back as written.
    let dir = std::env::temp_dir().join(format!("zk-test-verified-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let landed = write(&dir, &Found { manifest: &manifest, at: 1_790_726_400, kit: &kit, records: &records, fragment: Some(&fragment), emitters: &emitters, missed: &missed }).expect("written");
    assert_eq!(landed, path_of(&dir, &app::verifiedx::manifest_sha256(&manifest)));
    let bytes = std::fs::read(&landed).expect("there");
    assert_eq!(bytes, zikaron::json::canon_bytes(&held), "canonical bytes, keys in order at every depth");
    let back = zikaron::json::parse(&bytes).expect("reads back");
    let Some(Value::Arr(back_anchors)) = back.member("anchors") else { panic!("anchors") };
    assert_eq!(keys(&back_anchors[0]), ["blockNumber", "chainId", "hash", "registry", "tx"]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The read side's two pure judgments: whether a kit's stated anchoring point was read, and the basis of a
/// pass that left networks out.
#[test]
fn the_read_side_reads_what_is_added_and_names_its_unread_chains_in_the_basis() {
    use app::widex::{listed, with_unread_windows, Missed, Reading};
    use zikaron::json::Value;
    let reg = |b: u8| app::key::Address([b; 20]);
    let at = |chain: u64, registry: String| app::kitsindex::AnchoredOn { chain_id: chain, from_block: 0, registry };
    let main = app::auditx::Ground { chain: 8453, registry: reg(0x7a), from_block: 0, to_block: 0, senders: Vec::new() };
    let nets = vec![app::readnets::Net { chain_id: 42161, registry: reg(0x1a), from_block: 0, nodes: Vec::new(), name: None }];
    assert!(listed(None, None, &[]), "a kit that states nothing is always read");
    assert!(listed(Some(&at(8453, reg(0x7a).hex())), Some(&main), &[]), "the main network");
    assert!(listed(Some(&at(42161, reg(0x1a).hex())), None, &nets), "a read-only network");
    assert!(!listed(Some(&at(8453, reg(0x7b).hex())), Some(&main), &nets), "the same chain, another registry");
    assert!(!listed(Some(&at(10, reg(0x7a).hex())), Some(&main), &nets), "another chain");
    assert!(!listed(Some(&at(8453, reg(0x7a).hex())), None, &[]), "no network at all");
    assert!(!listed(Some(&at(8453, "0x12".into())), Some(&main), &nets), "a registry that does not read");
    let window = |chain: u64, from: u64, to: u64, regs: &[app::key::Address]| {
        Value::Obj(vec![
            ("chainId".into(), Value::Int(chain)),
            ("fromBlock".into(), Value::Int(from)),
            ("registries".into(), Value::Arr(regs.iter().map(|r| Value::Str(r.hex())).collect())),
            ("senders".into(), Value::Arr(Vec::new())),
            ("toBlock".into(), Value::Int(to)),
        ])
    };
    let fragment = Value::Obj(vec![("basis".into(), Value::Obj(vec![("chains".into(), Value::Arr(vec![window(8453, 0, 50, &[reg(0x7a)])]))]))]);
    assert_eq!(with_unread_windows(&fragment, &[], &[]), fragment, "nothing missed: unchanged");
    let miss = |chain: u64, r: u8, from: u64| Missed { chain_id: chain, registry: reg(r), from_block: from, name: String::new(), reading: Reading::Down };
    let senders = vec![format!("0x{}", "33".repeat(20))];
    let got = with_unread_windows(&fragment, &[miss(10, 0x3f, 9), miss(10, 0x3e, 4), miss(8453, 0x7b, 2)], &senders);
    let Some(Value::Arr(chains)) = got.member("basis").and_then(|b| b.member("chains")) else { panic!("chains") };
    let cell = |w: &Value, k: &str| w.member(k).cloned();
    assert!(chains.contains(&window(8453, 0, 50, &[reg(0x7a)])), "the window read stays as it was");
    let added: Vec<&Value> = chains.iter().filter(|w| cell(w, "registries") == Some(Value::Arr(Vec::new()))).collect();
    assert_eq!(chains.len(), 1 + added.len());
    let unread: Vec<(Option<Value>, Option<Value>, Option<Value>)> = added.iter().map(|w| (cell(w, "chainId"), cell(w, "fromBlock"), cell(w, "toBlock"))).collect();
    assert_eq!(unread.len(), 2, "each chain not read once");
    assert!(unread.contains(&(Some(Value::Int(10)), Some(Value::Int(4)), Some(Value::Int(4)))), "the lowest start block of that chain's networks: {unread:?}");
    assert!(unread.contains(&(Some(Value::Int(8453)), Some(Value::Int(51)), Some(Value::Int(51)))), "just past the window read on that chain: {unread:?}");
    for w in &added {
        assert_eq!(cell(w, "senders"), Some(Value::Arr(senders.iter().map(|s| Value::Str(s.clone())).collect())), "the senders the path scans for");
    }
}

/// The small-file write renames, then syncs the directory so the new name survives a power cut; a failed
/// directory sync fails the write.
#[test]
fn a_small_file_write_syncs_its_directory_after_the_rename() {
    let dir = std::env::temp_dir().join(format!("zk-put-sync-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    app::home::put_at(&dir, "a.json", b"1").expect("lands");
    assert_eq!(std::fs::read(dir.join("a.json")).expect("reads back"), b"1");
    std::fs::write(dir.join(".staged"), b"3").expect("a staged file");
    zikaron_os::pretend_sync_fails_at(Some(dir.clone()));
    let put = app::home::put_at(&dir, "a.json", b"2");
    let over = app::home::rename_over(&dir.join(".staged"), &dir.join("b.json"));
    zikaron_os::pretend_sync_fails_at(None);
    for (form, r) in [("putAt", put), ("renameOver", over)] {
        let f = r.err().unwrap_or_else(|| panic!("{form}: a directory that cannot be synced makes the write not done"));
        assert!(f.tail().contains(&dir.display().to_string()), "{form}: said with the directory: {}", f.tail());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A home's writer lock file is created owner-only, like the vault's lock, so another account can neither read
/// the holder from it nor open it to hold the lock. An existing file is opened as it is.
#[test]
fn a_homes_lock_file_is_created_owner_only() {
    // A writer's lock writes this machine's mark into the test's own machine directory, never the real one.
    vault_open();
    let dir = std::env::temp_dir().join(format!("zk-lock-owner-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = app::home::Home::open_or_create(&dir).expect("a home");
    let lock = app::lock::take(&home).expect("the lock");
    assert!(lock.mode().writable(), "the first instance writes");
    assert!(zikaron_os::is_owner_only(lock.path()).expect("reads back"), "the lock file is its owner's only");
    drop(lock);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A revert's fault appends what the contract said after the node's words, and the member stays
/// `CONTRACT_REFUSED`. A revert without data, and any other refusal, carry the node's words alone.
#[test]
fn a_revert_says_what_the_contract_said() {
    use app::fault::Known;
    use zikaron_anchor::rpc::Trouble;
    let mut data = vec![0x08, 0xc3, 0x79, 0xa0];
    data.extend([vec![0u8; 31], vec![32], vec![0u8; 31], vec![4], b"nope".to_vec(), vec![0u8; 28]].concat());
    let hex = zikaron::hexfmt::encode(&data);
    let with = Trouble::Node(format!("{{\"code\":3,\"message\":\"execution reverted\",\"data\":\"{hex}\"}}"));
    let f = app::chainx::said_fault("http://127.0.0.1:8545", &with);
    assert_eq!(f.which(), Some(Known::ContractRefused));
    assert!(f.tail().ends_with(" \u{b7} reason: nope"), "{}", f.tail());
    let bare = Trouble::Node("{\"code\":3,\"message\":\"execution reverted\"}".into());
    let f = app::chainx::said_fault("http://127.0.0.1:8545", &bare);
    assert_eq!(f.which(), Some(Known::ContractRefused));
    assert!(!f.tail().contains("reason:"), "{}", f.tail());
    let funds = Trouble::Node(format!("{{\"code\":-32000,\"message\":\"insufficient funds\",\"data\":\"{hex}\"}}"));
    assert!(!app::chainx::said_fault("http://127.0.0.1:8545", &funds).tail().contains("reason:"), "only a revert reads the data");
}

/// The same through a real request (`chainx::ask`) to an in-process node that refuses with revert data: the
/// fault is the contract's refusal and its evidence carries what the contract said.
#[test]
fn a_revert_asked_of_the_nodes_says_what_the_contract_said() {
    use std::io::{Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut s = s;
            let mut buf = vec![0u8; 8192];
            let _ = s.read(&mut buf);
            let mut data = vec![0x08, 0xc3, 0x79, 0xa0];
            data.extend([vec![0u8; 31], vec![32], vec![0u8; 31], vec![4], b"nope".to_vec(), vec![0u8; 28]].concat());
            let body = format!("{{\"error\":{{\"code\":3,\"message\":\"execution reverted\",\"data\":\"{}\"}},\"id\":1,\"jsonrpc\":\"2.0\"}}", zikaron::hexfmt::encode(&data));
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
        }
    });
    let eps = vec![app::chainx::Endpoint::at(31337, format!("http://127.0.0.1:{port}"))];
    let f = app::chainx::ask(&eps, "eth_estimateGas", &zikaron::json::Value::Arr(Vec::new())).err().expect("refused");
    assert_eq!(f.which(), Some(app::fault::Known::ContractRefused));
    assert!(f.tail().contains("reason: nope"), "{}", f.tail());
}

/// Machine settings change in one critical section (`machine::update`), so concurrent changes to different
/// cells never overwrite each other.
#[test]
fn two_machine_settings_changes_at_once_both_stay() {
    vault_open();
    let before = app::machine::read().expect("reads");
    let looks = std::thread::spawn(|| {
        for i in 0..40usize {
            let a = app::machine::APPEARANCES[i % app::machine::APPEARANCES.len()];
            app::machine::update(|m| m.appearance = Some(a.to_string())).expect("written");
        }
    });
    let start = before.auto_lock;
    let locks = std::thread::spawn(move || {
        for i in 1..=41u32 {
            app::machine::update(|m| m.auto_lock = if i % 2 == 1 { !start } else { start }).expect("written");
        }
    });
    looks.join().expect("joined");
    locks.join().expect("joined");
    let m = app::machine::read().expect("reads");
    let last_look = app::machine::APPEARANCES[39 % app::machine::APPEARANCES.len()];
    assert_eq!(m.appearance.as_deref(), Some(last_look), "the appearance's last change stayed");
    assert_eq!(m.auto_lock, !start, "the auto-lock switch's last change stayed");
    let (appearance, auto_lock) = (before.appearance.clone(), before.auto_lock);
    app::machine::update(|m| {
        m.appearance = appearance;
        m.auto_lock = auto_lock;
    })
    .expect("put back");
}

// ═════════════════════ Local data standard, second version ═════════════════════

/// A home in a temporary place, with its label made as a writer makes it.
fn labelled_home(tag: &str) -> app::home::Home {
    vault_open();
    let dir = std::env::temp_dir().join(format!("zk-std-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = app::home::Home::open_or_create(&dir).expect("建家");
    app::local::check_label(home.root(), None, true).expect("家标");
    home
}

fn local_reason(r: Result<Option<Vec<u8>>, app::fault::Fault>) -> Option<app::local::Unread> {
    match r {
        Err(f) => app::local::Unread::of(&f),
        Ok(_) => None,
    }
}

/// A sealed file moved to another home, or swapped under another name, is refused as `swapped` and left
/// untouched.
#[test]
fn a_file_put_in_another_home_or_under_another_name_is_another_file() {
    let (a, b) = (labelled_home("swap-a"), labelled_home("swap-b"));
    let room_a = a.dir(app::home::Slot::Settings);
    app::local::put(&room_a, app::settings::FILE, app::local::Doc::Settings, b"{\"capBytes\":7}").expect("落 a 的设置");
    let from = room_a.join(app::settings::FILE);
    let to = b.dir(app::home::Slot::Settings).join(app::settings::FILE);
    std::fs::copy(&from, &to).expect("换到 b");
    let before = std::fs::read(&to).unwrap();
    assert_eq!(local_reason(app::local::read(&to, app::local::Doc::Settings)), Some(app::local::Unread::Swapped));
    assert_eq!(std::fs::read(&to).unwrap(), before, "换来的档不动");
    assert!(app::local::read(&from, app::local::Doc::Settings).expect("原处照开").is_some());
    // Two verdicts in one home, each named by its grant, swapped under each other's names.
    let held = a.dir(app::home::Slot::GrantsHeld);
    let (g1, g2) = ("11".repeat(32), "22".repeat(32));
    for g in [&g1, &g2] {
        let at = app::lastread::verdict_path(&a, g).expect("判词的位置");
        let name = at.file_name().unwrap().to_string_lossy().to_string();
        let body = format!("{{\"grant\":\"{}\",\"verdict\":\"valid\"}}", app::lastread::grant_form(g));
        app::local::put(&held, &name, app::local::Doc::Verdict, body.as_bytes()).expect("落一份判词");
    }
    let names: Vec<std::path::PathBuf> = std::fs::read_dir(&held).unwrap().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.to_string_lossy().ends_with(app::lastread::VERDICT_SUFFIX)).collect();
    assert_eq!(names.len(), 2);
    let (x, y) = (std::fs::read(&names[0]).unwrap(), std::fs::read(&names[1]).unwrap());
    std::fs::write(&names[0], &y).unwrap();
    std::fs::write(&names[1], &x).unwrap();
    for n in &names {
        assert_eq!(local_reason(app::local::read(n, app::local::Doc::Verdict)), Some(app::local::Unread::Swapped), "{}", n.display());
    }
    let _ = std::fs::remove_dir_all(a.root());
    let _ = std::fs::remove_dir_all(b.root());
}

/// A file in the first envelope format (written by older versions) reads, and reading rewrites nothing; only
/// a write converts it to the second envelope.
#[test]
fn an_older_envelope_reads_and_is_rewritten_only_by_a_write() {
    let home = labelled_home("old-env");
    let at = home.dir(app::home::Slot::Settings).join(app::settings::FILE);
    let key = app::keybox::local_key().expect("库开着");
    let plain = b"{\"capBytes\":3}";
    let tag = app::local::Doc::Settings.tag().as_bytes();
    let mut head = app::local::MAGIC.to_vec();
    head.push(tag.len() as u8);
    head.extend_from_slice(tag);
    head.extend_from_slice(&1u16.to_be_bytes());
    let nonce = [9u8; 24];
    let mut old = head.clone();
    old.extend_from_slice(&nonce);
    old.extend_from_slice(&app::cryptx::xchacha_seal(key.bytes(), &nonce, &head, plain).unwrap());
    std::fs::write(&at, &old).unwrap();
    assert_eq!(app::local::read(&at, app::local::Doc::Settings).expect("旧封照开"), Some(plain.to_vec()));
    assert_eq!(std::fs::read(&at).unwrap(), old, "读不重写");
    app::local::put(&at.parent().unwrap(), app::settings::FILE, app::local::Doc::Settings, b"{\"capBytes\":4}").expect("写");
    assert!(std::fs::read(&at).unwrap().starts_with(app::local::MAGIC_V2), "写过即新版");
    let _ = std::fs::remove_dir_all(home.root());
}

/// An existing file that does not read is never overwritten: every overwrite kind refuses by name (reason and
/// way out in the tail) and leaves the bytes untouched; once the file is moved aside, the write creates it anew.
#[test]
fn a_file_that_does_not_read_is_never_written_over() {
    let home = labelled_home("not-over");
    let room = home.dir(app::home::Slot::Settings);
    let kinds = [
        (app::local::Doc::Settings, app::settings::FILE),
        (app::local::Doc::Queue, app::queue::FILE),
        (app::local::Doc::FirstWindow, app::wizard::FILE),
        (app::local::Doc::LastAudit, app::lastread::ANCHORED_FILE),
        (app::local::Doc::Unfetched, app::restorex::FILE),
    ];
    for (doc, name) in kinds {
        let at = room.join(name);
        for bad in [b"plain text".to_vec(), b"zikaron-local/2\n".to_vec(), {
            let id = app::local::Ident { owner: app::local::Owner::Home("ff".repeat(16)), doc, logical: format!("settings/{name}") };
            app::local::envelope(&[5u8; 32], &id, &[1u8; 24], b"{}").unwrap()
        }] {
            std::fs::write(&at, &bad).unwrap();
            let f = app::local::put(&room, name, doc, b"{}").expect_err("不盖读不成的档");
            assert_eq!(f.which(), Some(app::fault::Known::LocalSeal), "{}", doc.tag());
            assert!(app::local::Unread::of(&f).is_some() && f.tail().contains(&at.display().to_string()), "{}: {}", doc.tag(), f.tail());
            assert_eq!(std::fs::read(&at).unwrap(), bad, "{} 一字不动", doc.tag());
        }
        std::fs::remove_file(&at).unwrap();
        app::local::put(&room, name, doc, b"{}").expect("挪走之后即重建");
        assert!(app::local::read(&at, doc).expect("新的读得成").is_some());
    }
    let _ = std::fs::remove_dir_all(home.root());
}

/// A writer gives a home without a label its label (a number and whose it is); a reader makes none; a label of
/// another identity is refused as a different file when the home is opened as this one.
#[test]
fn a_writer_labels_a_home_and_a_label_of_another_identity_is_refused() {
    vault_open();
    let dir = std::env::temp_dir().join(format!("zk-std-label-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = app::home::Home::open_or_create(&dir).expect("建家");
    app::local::check_label(home.root(), None, false).expect("读者开");
    assert!(!app::local::label_path(home.root()).exists(), "读者不补家标");
    app::local::check_label(home.root(), None, true).expect("写者开");
    let l = app::local::read_label(home.root()).expect("家标读得成").expect("有家标");
    assert_eq!((l.number.len(), l.whose.clone()), (32, None));
    // A label written for identity 0xaa…'s author seat, opened as 0xbb…'s.
    let other = app::local::Label { number: l.number.clone(), whose: Some((format!("0x{}", "aa".repeat(20)), app::roles::Role::Author)) };
    let at = app::local::label_path(home.root());
    std::fs::write(&at, app::local::seal_with(&app::keybox::local_key().unwrap(), &app::local::label_ident(), &other.to_bytes()).unwrap()).unwrap();
    let id_b = format!("0x{}", "bb".repeat(20));
    let f = app::local::check_label(home.root(), Some((&id_b, app::roles::Role::Author)), true).expect_err("别人的家标");
    assert_eq!(app::local::Unread::of(&f), Some(app::local::Unread::Swapped));
    let id_a = format!("0x{}", "AA".repeat(20));
    app::local::check_label(home.root(), Some((&id_a, app::roles::Role::Author)), true).expect("本人的照开");
    // A label of nobody's home opened as an identity's seat: nobody is not another identity.
    let nobody = app::local::Label { number: l.number.clone(), whose: None };
    std::fs::write(&at, app::local::seal_with(&app::keybox::local_key().unwrap(), &app::local::label_ident(), &nobody.to_bytes()).unwrap()).unwrap();
    app::local::check_label(home.root(), Some((&id_b, app::roles::Role::Author)), true).expect("不属于任何身份的家照开");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A background task whose source has moved on writes nothing and ends with `HOME_UNREACHABLE`; kinds that do
/// not follow the source are not stopped.
#[test]
fn a_task_whose_home_moved_on_writes_nothing() {
    vault_open();
    let dir = std::env::temp_dir().join(format!("zk-std-ticket-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let run = |kind: app::task::Kind, move_on: bool, name: &'static str| -> (bool, Option<app::fault::Known>) {
        let mut t = app::task::Tasks::new();
        let (go_tx, go_rx) = std::sync::mpsc::channel::<()>();
        let d = dir.clone();
        let _ = t.spawn(kind, move || {
            let _ = go_rx.recv();
            app::home::put_at(&d, name, b"x").map(|_| app::task::Done::Reconciled { label: String::new(), complete: true, entries: 0 })
        });
        if move_on {
            t.new_epoch();
        }
        let _ = go_tx.send(());
        let mut said = None;
        for _ in 0..2000 {
            let got = t.drain_at(1.0);
            if let Some(o) = got.iter().find(|o| o.kind == kind) {
                said = o.result.as_ref().err().and_then(|f| f.which());
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        (dir.join(name).exists(), said)
    };
    assert_eq!(run(app::task::Kind::Record, true, "moved-on"), (false, Some(app::fault::Known::HomeUnreachable)));
    assert_eq!(run(app::task::Kind::Record, false, "same-epoch"), (true, None));
    assert_eq!(run(app::task::Kind::Anchor, true, "not-following"), (true, None));
    // The ledger's append is the other write gate: an entry a moved-on task would append is not written.
    let home = labelled_home("ticket-ledger");
    let ledger = app::local::ledger_of(&home).expect("账本");
    // An entry to append: the genesis a home of this test records (its plain bytes).
    let entry = {
        use app::action::{apply, Action, Applied};
        let src = std::env::temp_dir().join(format!("zk-std-ticket-src-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&src);
        let ctx = zikaron_ui::egui::Context::default();
        let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
        let _ = apply(&mut shell, Action::MakeAnchorKey);
        assert!(matches!(apply(&mut shell, Action::OpenHome { root: src.display().to_string() }), Applied::Homed { .. }));
        assert!(matches!(apply(&mut shell, Action::Genesis { statement: "票".into() }), Applied::Genesised { .. }));
        let got = shell.home.as_ref().and_then(|h| h.ledger().ok()).and_then(|l| l.pile().ok()).and_then(|p| p.items.first().cloned());
        drop(shell);
        let _ = std::fs::remove_dir_all(&src);
        got
    };
    assert!(entry.is_some(), "得一条条目");
    if let Some(bytes) = entry {
        let name = zikaron_store::EntryName::parse(zikaron::hexfmt::encode(&zikaron::entry::entry_id(&bytes)).trim_start_matches("0x")).expect("名");
        let mut t = app::task::Tasks::new();
        let (go_tx, go_rx) = std::sync::mpsc::channel::<()>();
        let root = ledger.root().to_path_buf();
        let n = name.clone();
        let _ = t.spawn(app::task::Kind::Record, move || {
            let _ = go_rx.recv();
            app::local::Ledger::open_or_create(root).and_then(|l| l.append(&n, &bytes)).map(|_| app::task::Done::Reconciled { label: String::new(), complete: true, entries: 0 })
        });
        t.new_epoch();
        let _ = go_tx.send(());
        let mut said = None;
        for _ in 0..2000 {
            if let Some(o) = t.drain_at(1.0).into_iter().find(|o| o.kind == app::task::Kind::Record) {
                said = o.result.as_ref().err().and_then(|f| f.which());
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(said, Some(app::fault::Known::HomeUnreachable), "账本追加收票");
        assert!(ledger.survey().expect("读").items.is_empty(), "一条也没落");
    }
    let _ = std::fs::remove_dir_all(home.root());
    // The window's own thread holds no ticket: its writes are never stopped.
    assert!(!app::task::ticket_void());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The ticket is checked only at the write gates (`home::put_at`, `home::land_numbered`, the ledger's append),
/// never by a task.
#[test]
fn the_ticket_is_asked_only_at_the_write_gates() {
    let mut at: Vec<String> = Vec::new();
    for (name, text) in shipped() {
        let n = code_only(&text).matches("ticket_void()").count();
        for _ in 0..n {
            at.push(name.clone());
        }
    }
    at.sort();
    assert_eq!(at, vec!["home.rs".to_string(), "home.rs".to_string(), "local.rs".to_string(), "task.rs".to_string()], "查票只在写口");
    let home = code_only(&read_src_file("home.rs").unwrap());
    let put_at = home.split("pub fn put_at(").nth(1).and_then(|b| b.split("\npub fn ").next()).unwrap_or_default();
    assert!(put_at.contains("ticket_void()"), "put_at 收票");
    let numbered = home.split("pub fn land_numbered(").nth(1).and_then(|b| b.split("\npub fn ").next()).unwrap_or_default();
    assert!(numbered.contains("ticket_void()"), "land_numbered 收票");
    let local = code_only(&read_src_file("local.rs").unwrap());
    let append = local.split("pub fn append(").nth(1).and_then(|b| b.split("\n    pub fn ").next()).unwrap_or_default();
    assert!(append.contains("ticket_void()"), "账本追加收票");
}

/// A home marked by another machine opens read-only as `OtherMachine` until the user takes it over; an
/// unreadable mark opens read-only and is kept.
#[test]
fn a_home_marked_by_another_machine_is_read_only_until_taken_over() {
    vault_open();
    let dir = std::env::temp_dir().join(format!("zk-std-mark-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = app::home::Home::open_or_create(&dir).expect("建家");
    let me = app::lock::this_machine().expect("本机标");
    assert_eq!(app::lock::this_machine().unwrap(), me, "本机标不变");
    {
        let l = app::lock::take(&home).expect("取锁");
        assert_eq!(l.mode(), app::lock::Mode::Writer);
        assert_eq!(app::lock::mark_of(&home), Some(me.clone()), "没有标即记本机");
    }
    let other = "0123456789abcdef0123456789abcdef";
    std::fs::write(app::lock::mark_path(&home), format!("{other}\n")).unwrap();
    let mut l = app::lock::take(&home).expect("取锁");
    assert_eq!(l.mode(), app::lock::Mode::OtherMachine);
    assert!(!l.mode().writable());
    l.take_over(&home).expect("改由本机写");
    assert_eq!((l.mode(), app::lock::mark_of(&home)), (app::lock::Mode::Writer, Some(me.clone())));
    l.take_over(&home).expect("已是写者,再按无事");
    drop(l);
    // A mark this version cannot read is no licence to write: read-only, named, the mark kept as it is (each
    // form is tested in `gaps_r20b1`).
    std::fs::write(app::lock::mark_path(&home), "not a mark\n").unwrap();
    assert_eq!(app::lock::take(&home).expect("取锁").mode(), app::lock::Mode::OtherMachine);
    assert_eq!(std::fs::read_to_string(app::lock::mark_path(&home)).unwrap(), "not a mark\n");
    assert_eq!(app::lock::mark_of(&home), None);
    let _ = me;
    let _ = std::fs::remove_dir_all(&dir);
}

/// A booted shell for the window probes (vault open, on this test process's own machine directory).
fn probe_shell(ctx: &zikaron_ui::egui::Context) -> app::shell::Shell {
    vault_open();
    app::shell::Shell::boot(zikaron_ui::skin::dress(ctx))
}

/// Shortcuts follow the key the keyboard layout produces: ⌘ with whichever key types "," opens settings,
/// wherever that key physically is.
#[test]
fn the_shortcuts_read_the_key_the_layout_gives_the_press() {
    use zikaron_ui::egui::{Event, Key, Modifiers};
    let settings = app::nav::Route::Root(app::nav::Place::SettingsHome).name();
    let home = app::nav::Route::Root(app::nav::Place::Home).name();
    let chord = |key: Key, physical: Option<Key>| Event::Key { key, physical_key: physical, pressed: true, repeat: false, modifiers: Modifiers::COMMAND };
    for (what, event, want) in [
        ("US: the key is , where , sits", chord(Key::Comma, Some(Key::Comma)), &settings),
        ("AZERTY: , is typed where M sits", chord(Key::Comma, Some(Key::M)), &settings),
        ("no physical key told", chord(Key::Comma, None), &settings),
        ("Dvorak: the key where , sits types W", chord(Key::W, Some(Key::Comma)), &home),
    ] {
        let ctx = zikaron_ui::egui::Context::default();
        let (_, said) = app::window::probe_shell_input(&ctx, probe_shell(&ctx), app::nav::Place::Home, vec![(0.1, vec![event]), (0.4, vec![]), (0.4, vec![])], 1180.0, 760.0);
        assert_eq!(&said.last().expect("frames ran").0, want, "{what}");
    }
    // A non-Latin layout (⌘ with the key where L sits on a Cyrillic layout) reaches egui as the physical key:
    // egui-winit reads `logical_key` and falls back to `physical_key`. The window adds no fallback of its own;
    // the egui-winit version is pinned here so an upgrade must recheck that fallback.
    let lock = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock")).expect("the lock file");
    let at = lock.find("name = \"egui-winit\"").expect("egui-winit in the lock");
    assert!(lock[at..].lines().nth(1).is_some_and(|l| l.trim() == "version = \"0.33.3\""), "egui-winit moved: check its logical-or-physical key fallback again");
}

/// The window's drag band (over the page) acts like a title bar: dragging moves the window, a double click
/// maximizes it, and a single click does neither.
#[test]
fn a_double_click_on_the_windows_handle_maximizes_it() {
    use zikaron_ui::egui::{pos2, Event, PointerButton};
    let at = pos2(zikaron_ui::tokens::RAIL_W + 200.0, 4.0);
    let press = |p, down| Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: down, modifiers: Default::default() };
    let sent = |steps: Vec<(f64, Vec<Event>)>| -> Vec<String> {
        // A fresh context per walk, so each starts its clock at zero.
        let ctx = zikaron_ui::egui::Context::default();
        let (_, said) = app::window::probe_shell_input(&ctx, probe_shell(&ctx), app::nav::Place::Home, steps, 1180.0, 760.0);
        said.into_iter().flat_map(|(_, c)| c).collect()
    };
    let one = vec![(0.1, vec![Event::PointerMoved(at)]), (0.1, vec![press(at, true), press(at, false)]), (0.5, vec![])];
    let two = vec![(0.1, vec![Event::PointerMoved(at)]), (0.05, vec![press(at, true), press(at, false)]), (0.1, vec![press(at, true), press(at, false)]), (0.5, vec![])];
    let drag = vec![(0.1, vec![Event::PointerMoved(at)]), (0.05, vec![press(at, true)]), (0.05, vec![Event::PointerMoved(pos2(at.x + 30.0, at.y + 20.0))]), (0.05, vec![Event::PointerMoved(pos2(at.x + 60.0, at.y + 40.0))]), (0.05, vec![press(pos2(at.x + 60.0, at.y + 40.0), false)])];
    let maxed = |c: &Vec<String>| c.iter().any(|s| s.starts_with("Maximized(true)"));
    let (one, two, drag) = (sent(one), sent(two), sent(drag));
    assert!(!maxed(&one), "one click maximizes nothing: {one:?}");
    assert!(maxed(&two), "a double click maximizes: {two:?}");
    assert!(drag.iter().any(|s| s == "StartDrag") && !maxed(&drag), "dragging moves the window: {drag:?}");
}

/// The window's minimum size is one constant derived from the layout's floors, and every page and settings
/// section fits it: nothing drawn runs past the right edge at that size.
#[test]
fn the_window_has_a_least_size_and_every_page_fits_it() {
    use app::nav::{Place, Section, View};
    use zikaron_ui::tokens;
    assert_eq!(app::window::MIN_W, tokens::RAIL_W + 2.0 * tokens::PAGE_PAD + tokens::MAIN_MIN_W, "the least width is the layout's floors");
    let faces = read_src_file("window/faces.rs").expect("faces");
    assert_eq!(code_only(&faces).matches(".with_min_inner_size([MIN_W, MIN_H])").count(), 1, "the window asks the system for that least size, once");
    let anywhere: usize = shipped().iter().map(|(_, t)| code_only(t).matches("with_min_inner_size").count()).sum();
    assert_eq!(anywhere, 1, "and nowhere else in what ships asks for another least size");
    let ctx = zikaron_ui::egui::Context::default();
    let mut places = vec![Place::Home, Place::SettingsHome];
    places.extend(View::ALL.iter().map(|v| Place::View(*v, 0)));
    places.extend(Section::ALL.iter().map(|s| Place::Settings(*s)));
    let mut shell = probe_shell(&ctx);
    for place in places {
        let (back, r) = app::window::probe_face(&ctx, shell, place, app::window::MIN_W, app::window::MIN_H);
        shell = back;
        assert!(r.right_text <= app::window::MIN_W + 0.5 && r.right_widget <= app::window::MIN_W + 0.5, "{place:?} runs past the least width: text to {}, widgets to {}", r.right_text, r.right_widget);
    }
}

/// A sentence's slots are filled in one pass: a value put in a slot is never read as a slot again, for one,
/// two or three slots (a path or typed name may contain "{1}").
#[test]
fn a_slot_is_filled_once_and_what_fills_it_is_never_read_again() {
    use app::lang::{fill1, fill2, fill3, t, Key};
    let with = |n: usize| Key::ALL.iter().copied().find(|k| (0..n).all(|i| t(*k).contains(&format!("{{{i}}}"))) && !t(*k).contains(&format!("{{{n}}}")));
    let k1 = with(1).expect("a sentence with one slot");
    let k2 = with(2).expect("a sentence with two slots");
    let k3 = with(3).expect("a sentence with three slots");
    let s = fill1(k1, "{0}{1}");
    assert!(s.contains("{0}{1}"), "slot 0's value stays as given: {s}");
    let s = fill2(k2, "a{1}b", "Z");
    assert!(s.contains("a{1}b") && s.matches('Z').count() == t(k2).matches("{1}").count(), "slot 1 is filled where the sentence has it, not inside slot 0's value: {s}");
    let s = fill3(k3, "{2}", "{1}", "C");
    assert!(s.contains("{2}") && s.contains("{1}") && s.contains('C'), "three slots, one pass: {s}");
    let s = fill1(k1, "");
    assert!(!s.contains("{0}"), "an empty value still fills its slot: {s}");
}

/// The dialogs a walk opened (what each allowed) and the answers still to come, for the stand-in asker of
/// [`the_file_dialog_does_not_hold_the_frame_and_answers_the_place_that_asked`].
static PATH_ASKED: std::sync::Mutex<Vec<app::platform::Pick>> = std::sync::Mutex::new(Vec::new());
static PATH_GATE: std::sync::Mutex<Option<std::sync::mpsc::Receiver<Result<Option<String>, app::fault::Fault>>>> = std::sync::Mutex::new(None);

/// Stands in for the system dialog: it opens (recorded) and its wait blocks until the walk answers.
fn stand_in_asker(kind: app::platform::Pick) -> Result<app::platform::Wait, app::fault::Fault> {
    PATH_ASKED.lock().unwrap().push(kind);
    let rx = PATH_GATE.lock().unwrap().take().expect("an answer channel for each dialog");
    Ok(Box::new(move || rx.recv().unwrap_or(Ok(None))))
}

/// A dialog that cannot open.
fn refusing_asker(_: app::platform::Pick) -> Result<app::platform::Wait, app::fault::Fault> {
    Err(app::fault::Fault::known(app::fault::Known::DialogUnavailable, String::new()))
}

/// The file dialog does not block the frame: the answer goes once to the place that asked, failures are
/// reported by name (never read as a cancel), and quitting does not wait for it.
#[test]
fn the_file_dialog_does_not_hold_the_frame_and_answers_the_place_that_asked() {
    use app::platform::Pick;
    use app::window::{probe_paths, PathFrame};
    let f = |ask: Vec<(&'static str, Pick)>, take: Vec<&'static str>, answer: bool| PathFrame { ask, take, answer };
    // Reported by name, once.
    let unavailable = |r: &app::window::PathRead| r.faults.iter().filter(|x| x.which() == Some(app::fault::Known::DialogUnavailable)).count() == 1;
    // Case A: chosen, delivered to the place that asked while frames kept running.
    let (tx, rx) = std::sync::mpsc::channel();
    *PATH_GATE.lock().unwrap() = Some(rx);
    PATH_ASKED.lock().unwrap().clear();
    let ctx = zikaron_ui::egui::Context::default();
    let mut send = || tx.send(Ok(Some("/somewhere/条款.pdf".to_string()))).unwrap();
    let mut frames = vec![f(vec![("a", Pick::File)], vec![], false), f(vec![("b", Pick::Folder)], vec!["a", "b"], false)];
    frames.extend((0..4).map(|_| f(vec![], vec!["a", "b"], false)));
    frames.push(f(vec![], vec!["b", "a"], true));
    frames.push(f(vec![], vec!["a"], false));
    let (_, r) = probe_paths(&ctx, probe_shell(&ctx), stand_in_asker, &mut send, frames);
    assert!(r[..6].iter().all(|x| x.open), "the dialog stays open while the window draws frame after frame");
    assert!(r[..6].iter().all(|x| x.took.iter().all(|(_, p)| p.is_none())), "nothing lands before the person answers");
    assert_eq!(*PATH_ASKED.lock().unwrap(), vec![Pick::File], "the second ask opened nothing; the first dialog kept its kind");
    assert_eq!(r[6].took, vec![("b", None), ("a", Some("/somewhere/条款.pdf".to_string()))], "the path goes back to the place that asked, and only there");
    assert!(!r[6].open);
    assert_eq!(r[7].took, vec![("a", None)], "taken once");
    // Case B: a cancel delivers nothing; the next request opens again.
    let (tx, rx) = std::sync::mpsc::channel();
    *PATH_GATE.lock().unwrap() = Some(rx);
    let (tx2, rx2) = std::sync::mpsc::channel();
    let mut sent = 0;
    let mut cancel = || {
        if sent == 0 {
            tx.send(Ok(None)).unwrap();
            *PATH_GATE.lock().unwrap() = Some(rx2.recv().unwrap());
        }
        sent += 1;
    };
    let (gate_tx, gate_rx) = std::sync::mpsc::channel();
    tx2.send(gate_rx).unwrap();
    let ctx = zikaron_ui::egui::Context::default();
    let (_, r) = probe_paths(&ctx, probe_shell(&ctx), stand_in_asker, &mut cancel, vec![f(vec![("a", Pick::Folder)], vec![], false), f(vec![], vec!["a"], true), f(vec![("a", Pick::FileOrFolder)], vec![], false)]);
    assert_eq!(r[1].took, vec![("a", None)], "a cancel lands nothing");
    assert!(!r[1].open && r[2].open, "and the next ask opens a dialog again");
    assert_eq!(PATH_ASKED.lock().unwrap().len(), 3);
    drop(gate_tx);
    // Case C: a dialog that cannot open is reported by name.
    let ctx = zikaron_ui::egui::Context::default();
    let (_, r) = probe_paths(&ctx, probe_shell(&ctx), refusing_asker, &mut || {}, vec![f(vec![("a", Pick::File)], vec!["a"], false), f(vec![], vec!["a"], false), f(vec![], vec![], false), f(vec![], vec![], false)]);
    assert!(unavailable(&r[0]) && !r[0].open && r[0].took == vec![("a", None)], "refused by name, nothing open, nothing landed");
    assert!(unavailable(&r[3]) && !r[3].open, "said once: the frames after it do not say it again");
    // Case D: a dialog whose wait fails (the portal's own failure) is reported by name, not read as a cancel.
    let (tx, rx) = std::sync::mpsc::channel();
    *PATH_GATE.lock().unwrap() = Some(rx);
    let ctx = zikaron_ui::egui::Context::default();
    let mut fail = || tx.send(Err(app::fault::Fault::known(app::fault::Known::DialogUnavailable, String::new()))).unwrap();
    let (_, r) = probe_paths(&ctx, probe_shell(&ctx), stand_in_asker, &mut fail, vec![f(vec![("a", Pick::File)], vec![], false), f(vec![], vec!["a"], true), f(vec![], vec!["a"], false), f(vec![], vec![], false)]);
    assert!(!unavailable(&r[0]) && unavailable(&r[1]) && r[1].took == vec![("a", None)], "said by name when it fails");
    assert!(unavailable(&r[3]), "said once: the frames after it do not say it again");
    // Case E: an answer no place takes is dropped.
    let (tx, rx) = std::sync::mpsc::channel();
    *PATH_GATE.lock().unwrap() = Some(rx);
    let ctx = zikaron_ui::egui::Context::default();
    let mut choose = || tx.send(Ok(Some("/somewhere/x".to_string()))).unwrap();
    let (_, r) = probe_paths(&ctx, probe_shell(&ctx), stand_in_asker, &mut choose, vec![f(vec![("a", Pick::File)], vec![], false), f(vec![], vec![], true), f(vec![], vec![], false), f(vec![], vec![], false), f(vec![], vec!["a"], false)]);
    assert_eq!(r[4].took, vec![("a", None)], "the place that asked was gone for those frames: the answer was dropped");
    // Quitting does not wait for an open dialog.
    assert!(!app::task::Kind::Path.waited_at_quit());
    // One way in: every place asks through the path mail; the system dialog is only the window's asker.
    let mut asks = 0;
    for (name, text) in shipped() {
        let code = code_only(&text);
        assert!(!code.contains("runModal"), "{name}: a modal dialog holds the frame");
        // The window's own files read as one (`window.rs`): the asker is named there once (below).
        let allowed = if name == "window.rs" { 1 } else { 0 };
        if name != "platform.rs" {
            assert_eq!(code.matches("platform::ask_path").count(), allowed, "{name} opens the dialog itself");
        }
        asks += code.matches("path_answer(").count();
    }
    let faces = code_only(&read_src_file("window/faces.rs").expect("faces"));
    assert_eq!(faces.matches("asker: crate::platform::ask_path").count(), 1, "the window's asker is the system's dialog");
    assert!(asks >= 16, "the places that offer \"choose…\" each ask through the mail: {asks}");
}

/// Readers of the path mail refuse by name a path that no longer exists or a folder where a file is wanted.
#[test]
fn a_chosen_path_that_is_gone_or_a_folder_where_a_file_is_wanted_is_refused_by_name() {
    let dir = std::env::temp_dir().join(format!("zk-chosen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("Package.app")).unwrap();
    let gone = app::anchorx::of_file(&dir.join("条款.pdf")).expect_err("a path no longer there");
    let folder = app::anchorx::of_file(&dir.join("Package.app")).expect_err("a folder where a file is wanted");
    assert_eq!(gone.which(), Some(app::fault::Known::FileMissing), "{}", gone.said());
    assert_eq!(folder.which(), Some(app::fault::Known::ContentShape), "{}", folder.said());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every task kind lands by its row in the one registry, and the window makes no landing decision of its own.
#[test]
fn every_landing_goes_through_one_registry() {
    use app::landing::{goes_back, of, Back};
    for k in app::task::Kind::ALL {
        assert_eq!(of(k).kind, k);
        assert_eq!(app::action::lands_said(k), goes_back(k, Back::Said), "{k:?}");
    }
    let landing = read_src_file("window/landing.rs").expect("the window's landing");
    let code = code_only(&landing);
    assert!(!code.contains("o.kind ==") && !code.contains("matches!(o.kind") && !code.contains("(o.kind, o.") && !code.contains("lands_said"), "the window's landing reads kinds through the registry");
    let draw = read_src_file("window/mod.rs").expect("window");
    assert!(code_only(&draw).contains("self.land(ctx, now);") && !code_only(&draw).contains("drain_at("), "draw takes its landings only through `land`");
    let shell = read_src_file("shell.rs").expect("shell");
    assert!(code_only(&shell).contains("crate::landing::goes_back(o.kind, crate::landing::Back::Vault)"), "the shell routes passcode answers by the registry");
}

/// After a failed chain read, the settings page's chain reading shows this failure (with why), not the reading
/// before it; once a read lands, the reading shows again.
#[test]
fn a_failed_chain_read_is_said_and_the_last_reading_not_shown() {
    use app::nav::{Place, Section};
    use app::task::{Done, Kind};
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = probe_shell(&ctx);
    let before_chain = shell.chain.clone();
    let before_failed = shell.failed.clone();
    // Read in whichever language the run uses (another test may switch it): the words come from the table.
    let failed_head = app::lang::t(app::lang::Key::SetReadFailedNow).split("{0}").next().unwrap_or("").to_string();
    let agreed = app::lang::fill1(app::lang::Key::SetAgreed, "3");
    shell.chain = Some(Done::Chain { gas_wei: Some(7), sources: 3, single_source: false, unanswered: Vec::new(), head_time: None });
    shell.failed.insert(Kind::Chain, app::fault::Fault::known(app::fault::Known::Unreachable, "http://node.invalid".to_string()));
    let (back, r) = app::window::probe_face(&ctx, shell, Place::Settings(Section::Network), 1180.0, 760.0);
    let mut shell = back;
    assert!(r.texts.iter().any(|t| t.starts_with(&failed_head)), "the failed read is said: {:?}", r.texts);
    assert!(!r.texts.iter().any(|t| t.contains(&agreed)), "the reading before it is not shown: {:?}", r.texts);
    shell.failed.remove(&Kind::Chain);
    let (back, r) = app::window::probe_face(&ctx, shell, Place::Settings(Section::Network), 1180.0, 760.0);
    let mut shell = back;
    assert!(r.texts.iter().any(|t| t.contains(&agreed)), "a landed reading shows: {:?}", r.texts);
    shell.chain = before_chain;
    shell.failed = before_failed;
}

/// The chain reading is read in one place (`Shell::chain_reading`, which reports a failed read): nothing else
/// reads the shell's `chain` field (writes stay where tasks land).
#[test]
fn the_chain_reading_is_read_in_one_place() {
    let mut stray = Vec::new();
    for (name, text) in shipped() {
        for (n, line) in code_only(&text).lines().enumerate() {
            for who in ["shell.chain", "self.chain"] {
                let mut from = 0;
                while let Some(at) = line[from..].find(who) {
                    let i = from + at + who.len();
                    from = i;
                    let next = line[i..].chars().next().unwrap_or(' ');
                    if next.is_alphanumeric() || next == '_' {
                        continue;
                    }
                    let rest = line[i..].trim_start();
                    let write = rest.starts_with('=') && !rest.starts_with("==");
                    let the_one = name == "shell.rs" && line.contains("Ok(self.chain.as_ref())");
                    // `self.chain` outside the shell is another type's own field.
                    if !write && !the_one && (who == "shell.chain" || name == "shell.rs") {
                        stray.push(format!("{name}:{} {}", n + 1, line.trim()));
                    }
                }
            }
        }
    }
    assert!(stray.is_empty(), "the chain reading read outside `chain_reading`: {stray:?}");
}

// ═════════════════════ Tests that need their own machine directory ═════════════════════
//
// The modules below each run every test alone in a process of its own ([`alone_in`]), on that process's own
// temporary machine directory, so a test that switches the language or makes identities touches no other.

/// Run test `test` of module `module` (its `module_path!()`) alone in a child process of this binary: `true` in
/// the parent (which waits for the child and requires it to pass), `false` in the child.
fn alone_in(module: &str, test: &str) -> bool {
    const CHILD: &str = "ZK_CHECKS_ALONE";
    let full = format!("{}::{test}", module.split_once("::").map(|(_, m)| m).unwrap_or(module));
    if std::env::var(CHILD).as_deref() == Ok(full.as_str()) {
        return false;
    }
    let me = std::env::current_exe().expect("this test binary");
    let out = output_of(std::process::Command::new(me).args([full.as_str(), "--exact", "--nocapture", "--test-threads=1"]).env(CHILD, &full)).expect("the child runs");
    assert!(out.status.success() && String::from_utf8_lossy(&out.stdout).contains("1 passed"), "{full}\n{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    true
}

#[path = "checks_more/gaps.rs"]
mod gaps;
#[path = "checks_more/gaps_b10m.rs"]
mod gaps_b10m;
#[path = "checks_more/window_b10.rs"]
mod window_b10;
#[path = "checks_more/window_b10_layout.rs"]
mod window_b10_layout;
#[path = "checks_more/gaps_b11.rs"]
mod gaps_b11;
#[path = "checks_more/gaps_b12.rs"]
mod gaps_b12;
#[path = "checks_more/gaps_b13.rs"]
mod gaps_b13;
#[path = "checks_more/door_b15.rs"]
mod door_b15;
#[path = "checks_more/gaps_r20b1.rs"]
mod gaps_r20b1;
#[path = "checks_more/gaps_r20b2.rs"]
mod gaps_r20b2;
#[path = "checks_more/a8_gates.rs"]
mod a8_gates;
#[path = "checks_more/a8_send.rs"]
mod a8_send;
#[path = "checks_more/a8_skeleton.rs"]
mod a8_skeleton;
#[path = "checks_more/gaps_r21b2.rs"]
mod gaps_r21b2;
