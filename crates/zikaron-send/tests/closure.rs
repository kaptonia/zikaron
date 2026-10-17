//! The sender carries no window and no third-party crate of its own: its whole dependency closure, as the lock
//! file records it, is the anchoring crate's and the sealed layer's, so the command line that depends on it stays
//! a command line.

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

/// The sender's closure is the anchoring crate's and the sealed layer's together, nothing more; neither it nor
/// the command line's holds a window.
#[test]
fn the_senders_closure_is_the_anchoring_crates_and_the_sealed_layers() {
    let sender = closure("zikaron-send");
    let mut under: BTreeSet<String> = closure("zikaron-anchor").union(&closure("zikaron-seal")).cloned().collect();
    under.insert("zikaron-send".into());
    assert_eq!(sender, under, "the sender reaches only what the anchoring crate and the sealed layer reach");
    for of in ["zikaron-send", "zikaron-cli"] {
        let all = closure(of);
        assert!(all.contains("zikaron-send"), "{of}: the closure was read: {all:?}");
        for window in ["egui", "eframe", "winit", "glow", "wgpu", "rfd", "raw-window-handle"] {
            assert!(!all.iter().any(|p| p == window || p.starts_with(&format!("{window}-")) || p.starts_with(&format!("{window}_"))), "{window} in {of}'s closure");
        }
    }
}

/// The sender names no third-party crate in its manifest: only the workspace's own, by path.
#[test]
fn the_sender_depends_on_no_third_party_crate() {
    let manifest = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).expect("the manifest");
    let deps = manifest.split("[dependencies]").nth(1).expect("dependencies");
    let third: Vec<&str> = deps.lines().map(str::trim).take_while(|l| !l.starts_with('[')).filter(|l| !l.is_empty() && !l.starts_with('#') && !l.contains("path =")).collect();
    assert!(third.is_empty(), "{third:?}");
}
