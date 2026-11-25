use std::path::{Path, PathBuf};

const BARRED: [&str; 36] = [
    "std::fs::",
    "File::open(",
    "File::create(",
    "read_dir(",
    ".metadata(",
    "symlink_metadata(",
    ".exists()",
    ".is_file()",
    ".is_dir()",
    "Home::open(",
    ".ledger()",
    ".pile()",
    "lock::mark_of(",
    "lock::take(",
    "lock::this_machine(",
    "local::read(",
    "register::read(",
    "json::parse(",
    "wire::parse(",
    "book::check(",
    "entry::check(",
    "cryptox::sha256(",
    "cryptox::keccak256(",
    "cryptox::recover",
    "secp256k1",
    "verify_kit",
    "canonicalize(",
    "door::place(",
    "machine_dir(",
    "kitx::preview",
    "kitx::originals(",
    "kitx::choose(",
    "grantfilex::open(",
    "grantfilex::is_grant_file(",
    "anchorx::file_size(",
    "recordsx::read(",
];

const FRAME_ROAD: [(&str, &str); 26] = [
    ("app/src/door.rs", "pub fn door_turn("),
    ("app/src/door.rs", "pub fn door_sync("),
    ("app/src/door.rs", "pub fn door_why("),
    ("app/src/door.rs", "fn door_open("),
    ("app/src/window/mod.rs", "fn turn(&mut self"),
    ("app/src/window/landing.rs", "pub(super) fn land("),
    ("app/src/window/watch.rs", "pub(super) fn tick("),
    ("app/src/shell.rs", "pub fn drain_at("),
    ("app/src/shell.rs", "pub fn credential_tick("),
    ("app/src/shell.rs", "pub fn turn_sync("),
    ("app/src/shell.rs", "pub fn book_changed("),
    ("app/src/shell.rs", "pub fn follow_disk("),
    ("app/src/shell.rs", "pub fn looked("),
    ("app/src/shell.rs", "pub fn keep_turn("),
    ("app/src/shell.rs", "fn kept(&mut self"),
    ("app/src/shell.rs", "fn queue_follows_ledger("),
    ("app/src/shell.rs", "pub fn seal_rows("),
    ("app/src/shell.rs", "pub fn sweep("),
    ("app/src/shell.rs", "pub fn keep_swept("),
    ("app/src/shell.rs", "fn fetch_landed("),
    ("app/src/shell.rs", "pub fn next_lane("),
    ("app/src/shell.rs", "pub fn carried_on("),
    ("app/src/action/rotate.rs", "fn faces_of("),
    ("app/src/action/rotate.rs", "pub fn rotation_stage("),
    ("app/src/shell.rs", "pub fn sendable_reading("),
    ("app/src/action/audit.rs", "pub(super) fn ground("),
];

const ROAD_BARRED: [&str; 27] = [
    "ledger_root(",
    "Queue::read(",
    "Settings::read(",
    "machine::read(",
    "register::view(",
    "register::change(",
    "ledgerx::rooted(",
    "entry_files(",
    "queue_mark(",
    "pens_mark(",
    "Seen::of(",
    "count_here(",
    ".regain(",
    "writer::take(",
    "take_turn(",
    ".write(h",
    "save_verdict(",
    "save_anchored(",
    "kitsindex::add(",
    "restorex::write(",
    "restorex::clear(",
    "restorex::standing(",
    "sendx::lift_met(",
    "queue::follow(",
    "queue::amend(",
    "reread_kits(",
    "aside_now(",
];

const ROAD_PRESSED: [(&str, &str, &str); 2] = [
    ("pub fn drain_at(", "reread_aside(", "a fetch the person pressed that failed after setting the home aside, or a fetch and replace they pressed: the old-data list read once where it lands"),
    ("pub fn drain_at(", "reopen_here(", "a fetch the person pressed that set the home aside: the fresh home at that place opened once where it lands"),
];

const ONCE: [(&str, &str); 1] = [("zikaron-ui/src/fonts.rs", "fonts are found and read once when the window starts")];

fn drawing_files(crates: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in [crates.join("app/src/window"), crates.join("zikaron-ui/src")] {
        let mut stack = vec![root];
        while let Some(d) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&d) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") && !p.ends_with("window/probes.rs") {
                    out.push(p);
                }
            }
        }
    }
    out.sort();
    out
}

fn code_lines(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with("#[cfg(test)]") {
            break;
        }
        if t.starts_with("//") {
            continue;
        }
        let code = line.split("//").next().unwrap_or("");
        out.push((i + 1, code));
    }
    out
}

fn barred_in(crates: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for f in drawing_files(crates) {
        let rel = f.strip_prefix(crates).unwrap_or(&f).to_string_lossy().replace('\\', "/");
        if ONCE.iter().any(|(p, _)| *p == rel) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        for (n, line) in code_lines(&text) {
            for b in BARRED {
                if line.contains(b) {
                    found.push(format!("{rel}:{n}: {b}"));
                }
            }
        }
    }
    found
}

fn barred_on_the_road(crates: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for (rel, opening) in FRAME_ROAD {
        let text = std::fs::read_to_string(crates.join(rel)).unwrap_or_else(|_| panic!("{rel}: the road names no file"));
        let at = text.find(opening).unwrap_or_else(|| panic!("{rel}: no {opening}"));
        let line_start = text[..at].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let indent: String = text[line_start..at].chars().take_while(|c| *c == ' ').collect();
        let close = format!("\n{indent}}}\n");
        let body = &text[at..text[at..].find(&close).map(|e| at + e).unwrap_or(text.len())];
        for (_, line) in code_lines(body) {
            for b in BARRED.iter().chain(ROAD_BARRED.iter()) {
                if line.contains(b) && !ROAD_PRESSED.iter().any(|(f, w, _)| *f == opening && w == b) {
                    found.push(format!("{rel}:{opening}: {b}"));
                }
            }
        }
    }
    found
}

#[test]
fn every_pressed_landing_on_the_road_is_there() {
    let crates = crates_dir();
    for (f, w, why) in ROAD_PRESSED {
        let (rel, _) = FRAME_ROAD.iter().find(|(_, o)| *o == f).unwrap_or_else(|| panic!("{f}: not on the road ({why})"));
        let text = std::fs::read_to_string(crates.join(rel)).expect("the file");
        assert!(text.contains(w), "{rel}: no {w}, the exception is stale ({why})");
    }
}

#[test]
fn the_road_every_pass_takes_reads_no_disk() {
    let found = barred_on_the_road(&crates_dir());
    assert!(found.is_empty(), "the road every pass takes reads the disk:\n{}", found.join("\n"));
}

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("the crates folder").to_path_buf()
}

#[test]
fn a_drawing_file_reads_no_disk_and_checks_nothing() {
    let found = barred_in(&crates_dir());
    assert!(found.is_empty(), "a frame does what a job should (move it into the reading the job builds):\n{}", found.join("\n"));
}

#[test]
fn every_exception_is_a_drawing_file_that_needs_it() {
    let crates = crates_dir();
    for (p, why) in ONCE {
        let text = std::fs::read_to_string(crates.join(p)).unwrap_or_else(|_| panic!("{p}: the exception names no file ({why})"));
        assert!(code_lines(&text).iter().any(|(_, l)| BARRED.iter().any(|b| l.contains(b))), "{p}: makes no barred call, the exception is stale ({why})");
    }
}

#[test]
fn a_read_put_in_a_page_is_caught() {
    let root = std::env::temp_dir().join(format!("frame-reads-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let page = root.join("app/src/window/others.rs");
    std::fs::create_dir_all(page.parent().expect("a folder")).expect("a folder");
    std::fs::create_dir_all(root.join("zikaron-ui/src")).expect("a folder");
    std::fs::write(&page, "fn draw() {\n    // std::fs::read in a comment is no call\n    let _ = std::fs::read(\"ledger\");\n}\n#[cfg(test)]\nmod tests { fn t() { let _ = std::fs::read(\"x\"); } }\n").expect("written");
    assert_eq!(barred_in(&root), vec!["app/src/window/others.rs:3: std::fs::".to_string()]);
    let _ = std::fs::remove_dir_all(&root);
}
