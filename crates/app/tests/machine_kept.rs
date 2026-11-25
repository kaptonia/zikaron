use std::path::{Path, PathBuf};

const ROLE: &str = "ZK_KEPT_ROLE";
const USER: &str = "ZK_KEPT_USER";

fn scratch(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("zk-kept-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("a scratch place");
    p
}

fn child(role: &str, user: &Path) -> Vec<String> {
    let me = std::env::current_exe().expect("this test binary");
    let out = std::process::Command::new(me)
        .args(["a_settled_machine_directory_is_kept_for_the_run", "--exact", "--nocapture", "--test-threads=1"])
        .env(ROLE, role)
        .env(USER, user)
        .output()
        .expect("the child runs");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success() && text.contains("1 passed"), "{role}: {}\n{text}\n{}", out.status, String::from_utf8_lossy(&out.stderr));
    text.lines().filter_map(|l| l.split_once("SAID ").map(|(_, said)| said.to_string())).collect()
}

fn steps(user: &Path) {
    app::places::set(app::places::Places { key_account: format!("zk-kept-{}", std::process::id()), machine_dir: None, user_home: Some(user.to_path_buf()) });
    let settled = app::home::settle_machine().expect("settled");
    let pointer = app::home::pointer_dir(user).join(app::home::POINTER);
    let elsewhere = user.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("another folder");
    std::fs::write(&pointer, app::home::machine_pointer_bytes(&elsewhere)).expect("the pointer edited by hand");
    let kept = app::home::machine_dir().expect("kept");
    println!("SAID kept={}", kept == settled.machine);
    std::fs::remove_file(&pointer).expect("the pointer removed");
    println!("SAID gone={}", app::home::machine_dir().map(|m| m == settled.machine).unwrap_or(false));
    app::home::forget_machine();
    std::fs::write(&pointer, app::home::machine_pointer_bytes(&elsewhere)).expect("the pointer edited by hand");
    println!("SAID fresh={}", app::home::machine_dir().map(|m| m == elsewhere).unwrap_or(false));
    app::home::forget_machine();
    std::fs::write(&pointer, app::home::machine_pointer_bytes(&settled.machine)).expect("the pointer put back");
    app::keybox::set_pin("27618394").expect("a vault laid");
    let home = settled.machine.join("a-home");
    std::fs::create_dir_all(&home).expect("a home");
    app::home::write_pointer(&home).expect("written");
    std::fs::write(&pointer, app::home::machine_pointer_bytes(&elsewhere)).expect("the pointer edited by hand");
    println!("SAID written={}", app::home::machine_dir().map(|m| m == settled.machine).unwrap_or(false));
}

#[test]
fn a_settled_machine_directory_is_kept_for_the_run() {
    if std::env::var_os(ROLE).is_some() {
        steps(&PathBuf::from(std::env::var(USER).expect("a user home")));
        return;
    }
    let user = scratch("user");
    let said = child("steps", &user);
    assert_eq!(said, vec!["kept=true", "gone=true", "fresh=true", "written=true"], "{said:?}");
    let _ = std::fs::remove_dir_all(&user);
}
