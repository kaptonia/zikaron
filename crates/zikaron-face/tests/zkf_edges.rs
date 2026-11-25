#[cfg(unix)]
use std::process::{Command, Stdio};
#[cfg(unix)]
use std::time::{Duration, Instant};
use zikaron::cryptox;
use zikaron::hexfmt;
use zikaron::json::Value;
use zikaron_face::{consent, names, shapes};

#[cfg(unix)]
const ZKF: &str = env!("CARGO_BIN_EXE_zkf");

#[cfg(unix)]
fn scratch(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("zkf-edges-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch");
    d
}

#[cfg(unix)]
fn zkf(args: &[&std::ffi::OsStr], limit: Duration) -> (Option<i32>, Vec<u8>, bool) {
    let mut child = Command::new(ZKF)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("zkf starts");
    let start = Instant::now();
    loop {
        if let Some(st) = child.try_wait().expect("wait") {
            let out = child.wait_with_output().expect("output").stdout;
            return (st.code(), out, false);
        }
        if start.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            return (None, Vec::new(), true);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(unix)]
fn os(s: &str) -> &std::ffi::OsStr {
    std::ffi::OsStr::new(s)
}

#[cfg(unix)]
#[test]
fn a_named_pipe_is_misuse_and_never_blocks() {
    let d = scratch("fifo");
    let p = d.join("pipe");
    let made = Command::new("mkfifo").arg(&p).status().expect("mkfifo runs");
    assert!(made.success(), "mkfifo");
    for cmd in ["carried", "table"] {
        let (code, out, timed_out) = zkf(&[os(cmd), p.as_os_str()], Duration::from_secs(10));
        assert!(!timed_out, "{cmd} blocked on a named pipe");
        assert_eq!(code, Some(2), "{cmd}");
        assert!(out.is_empty(), "{cmd} printed on misuse");
    }
    let root = "0x".to_string() + &"11".repeat(20);
    let (code, out, timed_out) = zkf(&[os("entry"), p.as_os_str(), os(&root)], Duration::from_secs(10));
    assert!(!timed_out && code == Some(2) && out.is_empty(), "entry on a pipe");
    let _ = std::fs::remove_dir_all(&d);
}

#[cfg(unix)]
#[test]
fn a_character_device_is_read_to_its_end() {
    let (code, out, _) = zkf(&[os("carried"), os("/dev/null")], Duration::from_secs(10));
    assert_eq!(code, Some(0));
    assert_eq!(out, br#"{"carried":null}"#);
    let (code, out, _) = zkf(&[os("table"), os("/dev/null")], Duration::from_secs(10));
    assert_eq!(code, Some(0));
    assert_eq!(out, br#"{"ok":false,"reason":"NO_TABLE"}"#);
}

#[cfg(unix)]
#[test]
fn the_sixteen_mebibyte_bound() {
    let (code, out, timed_out) = zkf(&[os("carried"), os("/dev/zero")], Duration::from_secs(60));
    assert!(!timed_out && code == Some(2) && out.is_empty(), "an endless device is misuse");
    let d = scratch("cap");
    let at = d.join("at.bin");
    std::fs::write(&at, vec![0u8; 16 * 1024 * 1024]).expect("write");
    let (code, out, _) = zkf(&[os("carried"), at.as_os_str()], Duration::from_secs(60));
    assert_eq!((code, out.as_slice()), (Some(0), br#"{"carried":null}"#.as_slice()), "the bound itself is read");
    let over = d.join("over.bin");
    std::fs::write(&over, vec![0u8; 16 * 1024 * 1024 + 1]).expect("write");
    let (code, out, _) = zkf(&[os("carried"), over.as_os_str()], Duration::from_secs(60));
    assert!(code == Some(2) && out.is_empty(), "past the bound is misuse");
    let _ = std::fs::remove_dir_all(&d);
}

#[cfg(unix)]
#[test]
fn an_argument_that_is_not_utf8_is_a_bad_argument() {
    use std::os::unix::ffi::OsStrExt;
    let bad = std::ffi::OsStr::from_bytes(b"0x\xff");
    let addr = "0x".to_string() + &"22".repeat(20);
    let (code, out, _) = zkf(&[os("declare"), bad, os(&addr), os("payout")], Duration::from_secs(10));
    assert!(code == Some(2) && out.is_empty());
    let (code, out, _) = zkf(&[os("declare"), os(&addr), os(&addr), bad], Duration::from_secs(10));
    assert!(code == Some(2) && out.is_empty());
}

fn key(n: u8) -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = n;
    k
}

fn key_body(root: &str, address: &str, role: &str, sig: &str) -> Value {
    let s = |x: &str| Value::Str(x.to_string());
    Value::Obj(vec![
        ("act".into(), s("bind")),
        ("root".into(), s(root)),
        ("kind".into(), s("key")),
        ("address".into(), s(address)),
        ("role".into(), s(role)),
        ("sig".into(), s(sig)),
    ])
}

#[test]
fn the_signer_s_consent_holds_under_its_root_alone() {
    let k = key(7);
    let address = hexfmt::encode(&cryptox::address_of_privkey(&k).expect("key"));
    let root = "0x".to_string() + &"ab".repeat(20);
    let other = "0x".to_string() + &"cd".repeat(20);
    let sig = consent::sig_hex(&consent::sign(&k, &root, &address, "payout").expect("signs"));
    let body = key_body(&root, &address, "payout", &sig);
    assert_eq!(shapes::shape_of(&body), Some(shapes::Shape::Key));
    assert!(consent::holds(&body));
    assert!(!consent::holds(&key_body(&other, &address, "payout", &sig)));
    assert!(!consent::holds(&key_body(&root, &address, "other-judge", &sig)));
}

#[test]
fn the_first_barred_value_is_named() {
    assert_eq!(names::first_barred("名字"), None);
    assert_eq!(names::first_barred("a\u{200D}b\u{FE0F}"), Some('\u{200D}'));
    assert_eq!(names::first_barred("x\u{10FFFE}"), Some('\u{10FFFE}'));
}
