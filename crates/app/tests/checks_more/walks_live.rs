//! Walking live directories (a restart's "File not found", `FaultWhatFileMissing`): the machine folder and a home
//! change while they are walked (a write's temporary file comes and goes; the command line's door is a socket
//! removed and bound again as the desktop opens a home). A listed entry gone by the time it is looked at is not
//! there, not an error: the whole-machine count and a home's usage read through it. Raced here against a thread
//! that keeps creating and removing files and folders beside what is walked; the place walked from must be there.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Files, and folders holding a file, appearing and vanishing in `dir` until `stop`, each under a fresh name.
fn churn(dir: std::path::PathBuf, stop: Arc<AtomicBool>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut i = 0u64;
        while !stop.load(Ordering::Relaxed) {
            let p = dir.join(format!("zk-churn-{i}"));
            let _ = std::fs::write(&p, b"x");
            let _ = std::fs::remove_file(&p);
            let d = dir.join(format!("zk-churn-dir-{i}"));
            let _ = std::fs::create_dir(&d);
            let _ = std::fs::write(d.join("inside"), b"x");
            let _ = std::fs::remove_dir_all(&d);
            i += 1;
        }
    })
}

#[test]
fn a_file_gone_while_the_machine_is_walked_is_not_an_error() {
    // Alone in its own process: the other tests of this binary make and remove homes on the same machine, and a
    // home the register names that is gone is an error of its own (it must be there).
    if super::alone_in(module_path!(), "a_file_gone_while_the_machine_is_walked_is_not_an_error") {
        return;
    }
    super::vault_open();
    let machine = app::home::machine_dir().expect("the machine folder");
    let stop = Arc::new(AtomicBool::new(false));
    let t = churn(machine.clone(), stop.clone());
    let failed = (0..400).filter_map(|_| app::local::all_files().err()).map(|f| f.raw()).next();
    stop.store(true, Ordering::Relaxed);
    let _ = t.join();
    assert!(failed.is_none(), "the walk failed on a file that came and went: {failed:?}");
}

#[test]
fn a_file_gone_while_a_home_is_measured_is_not_an_error() {
    let root = std::env::temp_dir().join(format!("zk-walks-live-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let home = app::home::Home::bare(root.clone());
    let settings = root.join("settings");
    std::fs::create_dir_all(&settings).expect("a room");
    std::fs::write(settings.join("kept"), b"0123456789").expect("a file that stays");
    let stop = Arc::new(AtomicBool::new(false));
    let t = churn(settings.clone(), stop.clone());
    let failed = (0..3000).filter_map(|_| home.usage().err()).map(|f| f.raw()).next();
    stop.store(true, Ordering::Relaxed);
    let _ = t.join();
    let after = home.usage();
    let _ = std::fs::remove_dir_all(&root);
    assert!(failed.is_none(), "the measure failed on a file that came and went: {failed:?}");
    assert_eq!(after.ok(), Some(10), "what stays is counted");
    // The home itself must be there: measured gone, it is an error, not nothing.
    assert!(home.usage().is_err(), "a home that is not there measures as an error");
}
