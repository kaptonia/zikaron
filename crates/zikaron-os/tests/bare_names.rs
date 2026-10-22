//! A write lands where it says, and syncs the directory it landed in, whatever shape the path has: a bare file
//! name and an empty directory sync the working directory `.`, a relative path with a directory and an absolute
//! path sync their parent (`zikaron_os::parent_dir`, the one place that says a path's directory). Each form runs
//! in a child process started in a fresh temporary directory, so the working directory is the child's own and
//! this process's never moves.

use std::path::{Path, PathBuf};
use std::process::Command;

const FORM: &str = "ZIKARON_OS_BARE_FORM";
const CHILD: &str = "the_child_writes_its_form_where_it_was_started";
const BYTES: &[u8] = b"landed";

/// The child's side: write the form named in `FORM` (with the sync of the expected directory made to fail when
/// asked) and say what came back. Started without `FORM` (the ordinary run), it does nothing.
#[test]
fn the_child_writes_its_form_where_it_was_started() {
    let Ok(form) = std::env::var(FORM) else { return };
    let (shape, pretend) = form.split_once(':').expect("form:mode");
    let here = std::env::current_dir().expect("the child's directory");
    let expected = match shape {
        "relative" => PathBuf::from("sub"),
        "absolute" => here.join("abs"),
        _ => PathBuf::from("."),
    };
    if pretend == "pretend" {
        zikaron_os::pretend_sync_fails_at(Some(expected.clone()));
    }
    let r = match shape {
        "bare" => {
            std::fs::write("tmp-bare", BYTES).expect("the temporary file");
            zikaron_os::rename_over(Path::new("tmp-bare"), Path::new("out.txt"))
        }
        "empty" => zikaron_os::put_at(Path::new(""), "out.txt", "tmp-empty", BYTES),
        "relative" => zikaron_os::put_at(Path::new("sub"), "out.txt", "tmp-rel", BYTES),
        "absolute" => zikaron_os::put_at(&here.join("abs"), "out.txt", "tmp-abs", BYTES),
        other => panic!("no form {other}"),
    };
    match r {
        Ok(()) => println!("REPORT ok"),
        Err(e) if e.at == expected => println!("REPORT unput at the expected directory"),
        Err(e) => println!("REPORT unput at={}", e.at.display()),
    }
}

/// Start the child in a fresh directory for `shape` and `mode`; its report and the directory.
fn child(shape: &str, mode: &str) -> (String, PathBuf) {
    let dir = std::env::temp_dir().join(format!("zk-os-bare-{shape}-{mode}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the child's directory");
    let out = Command::new(std::env::current_exe().expect("this test binary"))
        .args([CHILD, "--exact", "--nocapture", "--test-threads=1"])
        .env(FORM, format!("{shape}:{mode}"))
        .current_dir(&dir)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("the child runs");
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{shape}:{mode}: {said}\n{}", String::from_utf8_lossy(&out.stderr));
    let report = said.lines().find_map(|l| l.split_once("REPORT ").map(|(_, r)| r)).unwrap_or_else(|| panic!("{shape}:{mode} reported nothing: {said}")).to_string();
    (report, dir)
}

fn landed_at(shape: &str) -> &'static str {
    match shape {
        "relative" => "sub/out.txt",
        "absolute" => "abs/out.txt",
        _ => "out.txt",
    }
}

/// Each form lands its bytes, says it did and leaves no temporary file. Victim: whoever writes to a bare file name
/// and is told the write failed though it landed (a sync asked of the empty path).
#[test]
fn each_form_lands_and_says_so() {
    let before = std::env::current_dir().expect("this process's directory");
    for shape in ["bare", "empty", "relative", "absolute"] {
        let (report, dir) = child(shape, "plain");
        assert_eq!(report, "ok", "{shape}");
        assert_eq!(std::fs::read(dir.join(landed_at(shape))).expect("landed"), BYTES, "{shape}");
        let left: Vec<_> = std::fs::read_dir(dir.join(Path::new(landed_at(shape)).parent().unwrap())).unwrap().flatten().map(|e| e.file_name()).filter(|n| n.to_string_lossy().starts_with("tmp-")).collect();
        assert!(left.is_empty(), "{shape}: no temporary file left: {left:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
    assert_eq!(std::env::current_dir().expect("still here"), before, "this process's working directory never moves");
}

/// The directory synced is `.` for a bare name and an empty directory, the parent otherwise: making exactly that
/// directory's sync fail turns the answer into a failure said with it, and the file is in place all the same.
#[test]
fn each_form_syncs_the_directory_it_landed_in() {
    for shape in ["bare", "empty", "relative", "absolute"] {
        let (report, dir) = child(shape, "pretend");
        assert_eq!(report, "unput at the expected directory", "{shape}");
        assert_eq!(std::fs::read(dir.join(landed_at(shape))).expect("in place"), BYTES, "{shape}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn a_paths_directory_is_its_parent_or_the_working_directory() {
    use zikaron_os::parent_dir;
    assert_eq!(parent_dir(Path::new("out.txt")), Path::new("."));
    assert_eq!(parent_dir(Path::new("")), Path::new("."));
    assert_eq!(parent_dir(Path::new("./out.txt")), Path::new("."));
    assert_eq!(parent_dir(Path::new("sub/out.txt")), Path::new("sub"));
    assert_eq!(parent_dir(Path::new("/abs/out.txt")), Path::new("/abs"));
    assert_eq!(parent_dir(Path::new("/")), Path::new("/"), "a root is its own");
}
