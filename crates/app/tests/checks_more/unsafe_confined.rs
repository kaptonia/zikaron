//! `unsafe` lives in the platform pieces alone: the `zikaron-os` crate and the app's `platform` module. Every
//! other crate root of the product forbids it (the app denies it and lifts that for its `platform` module only;
//! the widget library forbids it in its shipped build, its own tests reading raw memory), and no source file
//! elsewhere under `crates/*/src` uses it.

fn crates() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Every `.rs` file under a directory.
fn sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(listing) = std::fs::read_dir(dir) else { return };
    for e in listing.flatten() {
        let p = e.path();
        if p.is_dir() {
            sources(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Whether a line uses `unsafe` as code (not in a comment).
fn uses_unsafe(line: &str) -> bool {
    let code = line.split("//").next().unwrap_or("");
    code.split(|c: char| !c.is_alphanumeric() && c != '_').any(|w| w == "unsafe")
}

#[test]
fn unsafe_is_only_in_the_platform_pieces() {
    let root = crates();
    let mut found = Vec::new();
    for c in std::fs::read_dir(&root).expect("crates").flatten() {
        let name = c.file_name().to_string_lossy().into_owned();
        if name == "zikaron-os" {
            continue;
        }
        let mut files = Vec::new();
        sources(&c.path().join("src"), &mut files);
        for f in files {
            let rel = f.strip_prefix(&root).unwrap_or(&f).to_string_lossy().replace('\\', "/");
            if rel.starts_with("app/src/platform/") {
                continue;
            }
            let text = std::fs::read_to_string(&f).expect("source");
            // The widget library's own tests read raw memory (its shipped build forbids it): the test module only.
            let shipped = if rel == "zikaron-ui/src/secret.rs" { text.split("#[cfg(test)]").next().unwrap_or("") } else { text.as_str() };
            for (i, line) in shipped.lines().enumerate() {
                if uses_unsafe(line) && !line.contains("unsafe_code") {
                    found.push(format!("{rel}:{}", i + 1));
                }
            }
        }
    }
    assert!(found.is_empty(), "unsafe outside the platform pieces: {found:?}");
}

#[test]
fn every_crate_root_forbids_unsafe_but_the_platform_piece() {
    let root = crates();
    let mut roots = Vec::new();
    for c in std::fs::read_dir(&root).expect("crates").flatten() {
        let src = c.path().join("src");
        for f in ["lib.rs", "main.rs"] {
            if src.join(f).is_file() {
                roots.push(src.join(f));
            }
        }
        if let Ok(bins) = std::fs::read_dir(src.join("bin")) {
            roots.extend(bins.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "rs")));
        }
    }
    assert!(roots.len() >= 18, "the crate roots were found: {roots:?}");
    for f in roots {
        let rel = f.strip_prefix(&root).unwrap_or(&f).to_string_lossy().replace('\\', "/");
        let text = std::fs::read_to_string(&f).expect("root");
        let want = match rel.as_str() {
            r if r.starts_with("zikaron-os/") => continue,
            "app/src/lib.rs" => "#![deny(unsafe_code)]",
            "zikaron-ui/src/lib.rs" => "#![cfg_attr(not(test), forbid(unsafe_code))]",
            _ => "#![forbid(unsafe_code)]",
        };
        assert!(text.lines().any(|l| l.trim() == want), "{rel} carries `{want}`");
    }
    let app = std::fs::read_to_string(root.join("app/src/lib.rs")).expect("app root");
    assert!(app.contains("#[allow(unsafe_code)]\npub mod platform;"), "the app lifts the denial for its platform module alone");
    assert_eq!(app.matches("allow(unsafe_code)").count(), 1, "and nowhere else");
}
