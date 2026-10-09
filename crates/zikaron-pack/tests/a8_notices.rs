//! The third-party licence summary has a single generator, checked by scanning the workspace's shipped sources
//! (crate sources and build scripts, the release tool, packaging scripts and workflows), read only.

use std::path::{Path, PathBuf};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every file under `dir` with one of `exts`, skipping build output.
fn walk(dir: &Path, exts: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.file_name().and_then(|x| x.to_str()) == Some("target") {
            continue;
        }
        if p.is_dir() {
            walk(&p, exts, out);
        } else if p.extension().and_then(|x| x.to_str()).is_some_and(|x| exts.contains(&x)) {
            out.push(p);
        }
    }
}

/// The shipped sources by workspace-relative path, with comment lines removed (Rust `//`, shell and workflow
/// `#`), so the scan sees code, not prose mentioning the same words.
fn shipped() -> Vec<(String, String)> {
    let root = workspace();
    let mut files = Vec::new();
    for e in std::fs::read_dir(root.join("crates")).expect("the crates").flatten() {
        let c = e.path();
        walk(&c.join("src"), &["rs"], &mut files);
        if c.join("build.rs").is_file() {
            files.push(c.join("build.rs"));
        }
    }
    walk(&root.join("tools/release/src"), &["rs"], &mut files);
    walk(&root.join("packaging"), &["sh", "py"], &mut files);
    walk(&root.join(".github"), &["yml", "yaml"], &mut files);
    let mut out: Vec<(String, String)> = files
        .into_iter()
        .map(|p| {
            let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()));
            let rust = p.extension().and_then(|x| x.to_str()) == Some("rs");
            let code = text
                .lines()
                .filter(|l| {
                    let t = l.trim_start();
                    if rust {
                        !t.starts_with("//")
                    } else {
                        !t.starts_with('#')
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            let rel = p.strip_prefix(&root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
            (rel, code)
        })
        .collect();
    out.sort();
    assert!(out.len() > 50, "the scan reached the sources ({} files)", out.len());
    out
}

fn holding(files: &[(String, String)], needle: &str) -> Vec<String> {
    files.iter().filter(|(_, c)| c.contains(needle)).map(|(n, _)| n.clone()).collect()
}

/// The third-party licence summary has exactly one generator, per target, honouring `license-file`: the only
/// code that asks cargo for a target-filtered dependency tree (`--filter-platform`) and the only code reading a
/// crate's `license_file` are in `crates/zikaron-pack/src/notices.rs`; `notices::make` is called only by the
/// app's build script (with the target being built), the packaging tool's `notices` command (`--target`
/// required) and the Windows package; and every packaging script that puts `THIRD-PARTY-LICENSES.txt` into a
/// package writes it with that command for a target, or installs the file so written.
#[test]
fn the_licence_summary_has_exactly_one_generator() {
    const GENERATOR: &str = "crates/zikaron-pack/src/notices.rs";
    let files = shipped();
    assert_eq!(holding(&files, "--filter-platform"), vec![GENERATOR], "the per-target dependency tree is asked in one place");
    assert_eq!(holding(&files, "license_file"), vec![GENERATOR], "a crate's `license-file` is read in one place");
    assert_eq!(holding(&files, "fn notices_of("), vec![GENERATOR], "the summary is assembled in one place");
    let generator = &files.iter().find(|(n, _)| n == GENERATOR).expect("the generator").1;
    assert!(generator.contains("\"--filter-platform\", target"), "the ask is filtered to the target it is given");
    assert_eq!(generator.matches("pub fn make(").count(), 1, "one entry");
    // Its callers: each passes a target.
    let mut callers = holding(&files, "notices::make(");
    callers.sort();
    // The release tool, where it is present, makes the repository's root summary for one named target.
    if let Some(i) = callers.iter().position(|c| c == "tools/release/src/main.rs") {
        let release = &files.iter().find(|(n, _)| n == "tools/release/src/main.rs").expect("the release tool").1;
        assert!(release.contains("notices::make(\"cargo\", &out.join(\"Cargo.toml\"), \"aarch64-apple-darwin\","), "the release tool makes the root summary for a named target");
        callers.remove(i);
    }
    assert_eq!(callers, vec!["crates/app/build.rs", "crates/zikaron-pack/src/main.rs", "crates/zikaron-pack/src/windows.rs"], "the generator's callers");
    let of = |n: &str| files.iter().find(|(f, _)| f == n).map(|(_, c)| c.as_str()).unwrap_or("");
    assert!(of("crates/app/build.rs").contains("std::env::var(\"TARGET\")"), "the app's build script makes it for the target being built");
    assert!(of("crates/zikaron-pack/src/main.rs").contains("\"--target is required\""), "the packaging tool's command wants a target");
    let windows = of("crates/zikaron-pack/src/windows.rs");
    assert!(windows.contains("crate::notices::make(&cargo, &root.join(\"Cargo.toml\"), target,"), "the Windows package makes it for its target");
    // Packaging scripts: a summary placed in a package is the command's `--out`, or a copy of it.
    let mut scripts = 0;
    for (name, code) in files.iter().filter(|(n, _)| n.starts_with("packaging/")) {
        let lines: Vec<&str> = code.lines().filter(|l| l.contains("THIRD-PARTY-LICENSES.txt")).collect();
        if lines.is_empty() {
            continue;
        }
        scripts += 1;
        assert!(code.contains("\"$PACK\" notices --target "), "{name} makes the summary with the packaging tool, for a target");
        let outs: Vec<&str> = lines
            .iter()
            .filter_map(|l| l.split("--out ").nth(1))
            .map(|s| s.trim().trim_end_matches('\\').trim())
            .collect();
        assert!(!outs.is_empty(), "{name} writes the summary as the command's output");
        for l in lines {
            assert!(l.contains("--out ") || outs.iter().any(|o| l.contains(o)), "{name}: {l:?} lays a summary the command did not write");
        }
    }
    assert!(scripts >= 2, "the macOS and Linux scripts were read ({scripts})");
}
