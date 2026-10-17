//! The sealed layer carries no window: its whole dependency closure, as the lock file records it, holds none of
//! the window family, so the command line that depends on it stays a command line. Its third-party
//! dependencies are the closed seven of its manifest.

use std::collections::{BTreeMap, BTreeSet};

/// Each package of the lock file and the names of the packages it depends on.
fn lock() -> BTreeMap<String, Vec<String>> {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock")).expect("the lock file");
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for block in text.split("[[package]]").skip(1) {
        let name = block.lines().find_map(|l| l.strip_prefix("name = \"")).and_then(|r| r.strip_suffix('"')).expect("a package name").to_string();
        let deps: Vec<String> = match block.split_once("dependencies = [") {
            Some((_, rest)) => rest
                .split(']')
                .next()
                .unwrap_or_default()
                .lines()
                .filter_map(|l| l.trim().trim_end_matches(',').strip_prefix('"').and_then(|x| x.strip_suffix('"')))
                .map(|d| d.split(' ').next().unwrap_or_default().to_string())
                .collect(),
            None => Vec::new(),
        };
        out.entry(name).or_default().extend(deps);
    }
    out
}

fn closure(of: &str) -> BTreeSet<String> {
    let lock = lock();
    let mut seen = BTreeSet::new();
    let mut todo = vec![of.to_string()];
    while let Some(n) = todo.pop() {
        if seen.insert(n.clone()) {
            todo.extend(lock.get(&n).cloned().unwrap_or_default());
        }
    }
    seen
}

#[test]
fn the_sealed_layers_closure_holds_no_window() {
    let all = closure("zikaron-seal");
    assert!(all.contains("chacha20poly1305"), "the closure was read: {all:?}");
    for window in ["egui", "eframe", "winit", "glow", "wgpu", "rfd", "raw-window-handle"] {
        assert!(!all.iter().any(|p| p == window || p.starts_with(&format!("{window}-")) || p.starts_with(&format!("{window}_"))), "{window} in the sealed layer's closure");
    }
}

#[test]
fn the_sealed_layers_third_party_crates_are_the_closed_seven() {
    let manifest = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).expect("the manifest");
    let deps = manifest.split("[dependencies]").nth(1).expect("dependencies");
    let third: BTreeSet<&str> = deps
        .lines()
        .map(str::trim)
        .take_while(|l| !l.starts_with('['))
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.contains("path ="))
        .filter_map(|l| l.split(" =").next())
        .collect();
    assert_eq!(third, BTreeSet::from(["aes", "chacha20poly1305", "ctr", "hkdf", "hmac", "scrypt", "sha2"]));
}
