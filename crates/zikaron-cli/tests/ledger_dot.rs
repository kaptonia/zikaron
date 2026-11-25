use std::path::{Path, PathBuf};
use zikaron_cli::sealed::{is_home, ledger_given};

fn a_home(at: &Path) {
    let label = zikaron_seal::envelope::label_path(at);
    std::fs::create_dir_all(label.parent().expect("settings")).expect("settings");
    std::fs::write(&label, b"{}").expect("a label");
    std::fs::create_dir_all(at.join(zikaron_seal::envelope::LEDGER_DIR)).expect("its ledger folder");
}

#[test]
fn a_dot_in_the_data_folder_is_the_data_folder() {
    let base = std::env::temp_dir().join(format!("zk-cli-ledger-dot-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let home = base.join("author");
    a_home(&home);
    let plain = base.join("plain");
    std::fs::create_dir_all(&plain).expect("a plain folder");
    std::env::set_current_dir(&home).expect("into the data folder");
    let here = std::env::current_dir().expect("the working directory");
    let ledger = here.join(zikaron_seal::envelope::LEDGER_DIR);
    for form in [".", "./"] {
        let got = ledger_given(form);
        assert_eq!(PathBuf::from(&got), ledger, "{form}: its ledger folder");
        assert!(is_home(&got), "{form}: a home");
    }
    assert_eq!(PathBuf::from(ledger_given(&here.display().to_string())), ledger, "the whole path, alike");
    assert_eq!(ledger_given("ledger"), "ledger", "a bare ledger folder, as given");
    assert!(is_home("ledger"), "and a home");
    std::env::set_current_dir(&plain).expect("into a plain folder");
    assert_eq!(ledger_given("."), ".", "a plain folder, as given");
    assert!(!is_home("."), "no home");
    let _ = std::env::set_current_dir(std::env::temp_dir());
    let _ = std::fs::remove_dir_all(&base);
}
