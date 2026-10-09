//! The packaging scripts and the CI step take path remappings from one table (`zikaron-pack rustflags`, built
//! from `windows::remaps`), and the AppImage is assembled with no tool other than the pinned runtime.

use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

const SCRIPTS: [&str; 3] = ["packaging/linux/build.sh", "packaging/linux/cross-build.sh", "packaging/macos/build.sh"];

/// No script or workflow spells out a path remapping itself; every script, and the CI build that checks for
/// runner paths, takes the flags from `zikaron-pack rustflags`.
#[test]
fn the_path_mappings_are_spelled_in_one_place() {
    let mut spelled = Vec::new();
    for dir in ["packaging/linux", "packaging/macos", ".github/workflows"] {
        for e in std::fs::read_dir(root().join(dir)).expect("folder").flatten() {
            let text = std::fs::read_to_string(e.path()).unwrap_or_default();
            if text.contains("--remap-path-prefix") {
                spelled.push(e.path().display().to_string());
            }
        }
    }
    assert!(spelled.is_empty(), "path mappings spelled outside the one table: {spelled:?}");
    for s in SCRIPTS {
        assert!(read(s).contains("rustflags --target-dir"), "{s} takes the flags from the one table");
    }
    assert!(read(".github/workflows/test.yml").contains("zikaron-pack rustflags --target-dir"), "the CI step too");
}

/// Both Linux scripts build the AppImage from the runtime checked against its architecture's pin plus a
/// squashfs image; no `appimagetool` (an unpinned build bundling its own runtime) is used.
#[test]
fn the_appimage_is_made_of_the_pinned_runtime_only() {
    for s in ["packaging/linux/build.sh", "packaging/linux/cross-build.sh"] {
        let text = read(s);
        let code: String = text.lines().filter(|l| !l.trim_start().starts_with('#')).collect::<Vec<_>>().join("\n");
        assert!(!code.contains("appimagetool"), "{s} runs no appimagetool");
        assert!(code.contains("appimage-runtime \"$RUNTIME\" --arch"), "{s} checks the runtime against its architecture's pin");
        assert!(code.contains("mksquashfs") && code.contains("cat \"$RUNTIME\" \"$WORK/image.squashfs\""), "{s} lays the image after the runtime");
    }
}

/// The Linux build records the .deb's checksum as soon as the .deb is made, before attempting the AppImage: an
/// architecture with no pinned runtime (refused by name) or a failed AppImage step still leaves the .deb with
/// its checksum, and the build exits non-zero.
#[test]
fn the_deb_is_summed_before_the_appimage_is_tried() {
    let text = read("packaging/linux/build.sh");
    let deb_sum = text.find("sha256sum \"$(basename \"$DEB\")\" >").expect("the .deb summed on its own");
    let deb = text.find("dpkg-deb --root-owner-group --build").expect("the .deb is built");
    let runtime = text.find("appimage-runtime \"$RUNTIME\" --arch").expect("the runtime is checked");
    assert!(deb < deb_sum && deb_sum < runtime, "summed after it is built and before the AppImage step");
    assert!(text.contains("set -euo pipefail"), "a refused step stops the build non-zero");
}
