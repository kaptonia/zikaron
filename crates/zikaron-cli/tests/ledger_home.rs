use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_zikaron");

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch(tag: &str) -> Scratch {
    let p = std::env::temp_dir().join(format!("zk-cli-ledger-home-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("a scratch place");
    Scratch(p)
}

fn run(dir: &Path, user: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .env(zikaron_os::HOME_VAR, user)
        .env("LANG", "en_US.UTF-8")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("LANGUAGE")
        .output()
        .expect("runs");
    let err = String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or_default().to_string();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), err)
}

fn reason(out: &str) -> String {
    zikaron::json::parse(out.as_bytes()).ok().and_then(|v| v.member("reason").and_then(|r| r.as_str()).map(str::to_string)).unwrap_or_default()
}

fn a_home(at: &Path) {
    let label = zikaron_seal::envelope::label_path(at);
    std::fs::create_dir_all(label.parent().expect("settings")).expect("settings");
    std::fs::write(&label, b"{}").expect("a label");
    std::fs::create_dir_all(at.join(zikaron_seal::envelope::LEDGER_DIR)).expect("its ledger folder");
}

#[test]
fn the_data_folder_and_its_ledger_folder_are_one_home() {
    let w = scratch("forms");
    let user = w.0.join("user");
    std::fs::create_dir_all(&user).expect("user home");
    let home = w.0.join("author");
    a_home(&home);
    std::fs::write(w.0.join("w.txt"), b"a work").expect("a work");
    let file = w.0.join("w.txt").display().to_string();
    let ledger = home.join("ledger");
    let forms: Vec<(&str, PathBuf, String)> = vec![
        ("data folder", w.0.clone(), home.display().to_string()),
        ("data folder/", w.0.clone(), format!("{}/", home.display())),
        ("ledger folder", w.0.clone(), ledger.display().to_string()),
        ("ledger folder/", w.0.clone(), format!("{}/", ledger.display())),
        ("bare ledger", home.clone(), "ledger".to_string()),
    ];
    for (form, cwd, at) in &forms {
        let verbs: Vec<Vec<&str>> = vec![
            vec!["history", "--ledger", at, "--file", &file],
            vec!["audit", "--ledger", at],
            vec!["anchor", "--ledger", at],
            vec!["scan", "--ledger", at],
        ];
        for args in verbs {
            let (code, out, err) = run(cwd, &user, &args);
            assert_eq!((code, reason(&out)), (1, "E_CREDENTIAL_MISSING".to_string()), "{form} {}: {out} {err}", args[0]);
        }
    }
    let plain = w.0.join("plain");
    std::fs::create_dir_all(&plain).expect("a plain folder");
    let (code, _, err) = run(&w.0, &user, &["history", "--ledger", &plain.display().to_string(), "--file", &file]);
    assert_eq!((code, err.as_str()), (2, "E_ARGS --key"), "a plain folder takes its key");
}

#[test]
fn init_leaves_a_home_to_the_desktop() {
    const KEY: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
    let w = scratch("init");
    let user = w.0.join("user");
    std::fs::create_dir_all(&user).expect("user home");
    for (form, home, given) in [("data folder", w.0.join("a"), None), ("ledger folder", w.0.join("b"), Some("ledger"))] {
        a_home(&home);
        let at = given.map_or_else(|| home.display().to_string(), |l| home.join(l).display().to_string());
        let (code, out, err) = run(&w.0, &user, &["init", "--ledger", &at, "--key", KEY, "--statement", "s"]);
        assert_eq!((code, reason(&out)), (1, "E_ON_DESKTOP".to_string()), "{form}: {out} {err}");
        let left = std::fs::read_dir(home.join("ledger")).expect("its ledger folder").count();
        assert_eq!(left, 0, "{form}: nothing written");
    }
    let plain = w.0.join("plain");
    let (code, out, err) = run(&w.0, &user, &["init", "--ledger", &plain.display().to_string(), "--key", KEY, "--statement", "s"]);
    assert_eq!(code, 0, "a plain folder takes its genesis: {out} {err}");
}
