//! A file landed by a bare name lands in the working directory and syncs `.`, through the one place that says a
//! path's directory (`zikaron_os::parent_dir`). Run in a child process started in a fresh temporary directory,
//! so this process's working directory never moves.

use std::process::Command;
use zikaron_glue::landing::{Trouble, land_bytes};

const FORM: &str = "ZIKARON_GLUE_BARE_FORM";
const CHILD: &str = "the_child_lands_a_bare_name_where_it_was_started";

/// The child's side: land `out.json` by its bare name (with the sync of `.` made to fail when asked) and say
/// what came back. Started without `FORM` (the ordinary run), it does nothing.
#[test]
fn the_child_lands_a_bare_name_where_it_was_started() {
    let Ok(mode) = std::env::var(FORM) else { return };
    if mode == "pretend" {
        zikaron_os::pretend_sync_fails_at(Some(".".into()));
    }
    match land_bytes(std::path::Path::new("out.json"), b"landed") {
        Ok(()) => println!("REPORT ok"),
        Err(Trouble::Io(subject, said)) => println!("REPORT io {subject} | {said}"),
        Err(t) => println!("REPORT {}", t.code()),
    }
}

fn child(mode: &str) -> (String, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("zk-glue-bare-{mode}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the child's directory");
    let out = Command::new(std::env::current_exe().expect("this test binary"))
        .args([CHILD, "--exact", "--nocapture", "--test-threads=1"])
        .env(FORM, mode)
        .current_dir(&dir)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("the child runs");
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{mode}: {said}\n{}", String::from_utf8_lossy(&out.stderr));
    (said.lines().find_map(|l| l.split_once("REPORT ").map(|(_, r)| r)).unwrap_or_else(|| panic!("{mode} reported nothing: {said}")).to_string(), dir)
}

/// The bare name lands, says it did, and leaves no temporary file; with the sync of `.` made to fail, the
/// landing is said failed naming `.`, the bytes in place.
#[test]
fn a_bare_name_lands_in_the_working_directory_and_syncs_it() {
    let before = std::env::current_dir().expect("this process's directory");
    let (report, dir) = child("plain");
    assert_eq!(report, "ok");
    assert_eq!(std::fs::read(dir.join("out.json")).expect("landed"), b"landed");
    let names: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["out.json"], "no temporary file left");
    let _ = std::fs::remove_dir_all(&dir);
    let (report, dir) = child("pretend");
    assert!(report.starts_with("io ") && report.contains("directory sync .:"), "{report}");
    assert_eq!(std::fs::read(dir.join("out.json")).expect("in place"), b"landed");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(std::env::current_dir().expect("still here"), before);
}
