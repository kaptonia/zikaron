//! The Windows package: the window binary and the command line built for one Windows target, with the building
//! machine's paths (its home, cargo and rustup homes, the checkout and the build directory) written as neutral
//! names in what the compiler records, laid out in one folder with the licence and the third-party notices
//! made for that target. No installer or archive yet.
//!
//! The build is the same `cargo build` anyone runs; only the path mapping is added (as the macOS and Linux
//! packages do), and on an MSVC target the C runtime is linked in (`+crt-static`), so the programs start on a
//! machine without the Visual C++ runtime installed. The mapping goes in the encoded form cargo reads (`CARGO_ENCODED_RUSTFLAGS`), so a path with
//! a space in it stays one argument.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The target the Windows package is built for.
pub const TARGET: &str = "x86_64-pc-windows-msvc";

/// The fonts every build but macOS's embeds, whose licences follow the crates' in the notices.
pub const FONT_LICENCES: &[&str] = &["crates/zikaron-ui/fonts/OFL-Inter.txt", "crates/zikaron-ui/fonts/OFL-JetBrainsMono.txt", "crates/zikaron-ui/fonts/OFL-NotoSansSC.txt"];

/// The version the app's manifest says.
fn version(root: &Path) -> Result<String, String> {
    let manifest = std::fs::read_to_string(root.join("crates/app/Cargo.toml")).map_err(|e| format!("crates/app/Cargo.toml: {e}"))?;
    manifest
        .lines()
        .find_map(|l| l.strip_prefix("version").map(|r| r.trim().trim_start_matches('=').trim().trim_matches('"').to_string()))
        .ok_or_else(|| "no version in crates/app/Cargo.toml".to_string())
}

/// The path mappings for this machine: each place on the left is recorded as the name on the right. The
/// compiler applies the last mapping that matches, so the home directory comes first and every narrower place
/// under it (cargo's and rustup's homes, the checkout, the build directory) keeps its own name.
pub fn remaps(root: &Path, target_dir: &Path) -> Vec<(PathBuf, &'static str)> {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let home = var("HOME").or_else(|| var("USERPROFILE"));
    let mut out = Vec::new();
    if let Some(h) = home.clone() {
        out.push((h, "/home"));
    }
    if let Some(c) = var("CARGO_HOME").or_else(|| home.as_ref().map(|h| h.join(".cargo"))) {
        out.push((c, "/cargo"));
    }
    if let Some(r) = var("RUSTUP_HOME").or_else(|| home.as_ref().map(|h| h.join(".rustup"))) {
        out.push((r, "/rustup"));
    }
    out.push((root.to_path_buf(), "/zikaron"));
    out.push((target_dir.to_path_buf(), "/target"));
    out
}

/// A path without `.` and `..` steps, spelled as the compiler compares prefixes (step by step, never through
/// the disk): `crates/zikaron-pack/../..` is the workspace root.
pub fn plain(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// The compiler flags cargo is given, in its encoded form: what the environment already asks
/// (`CARGO_ENCODED_RUSTFLAGS`, else `RUSTFLAGS` split at whitespace), then the path mappings.
pub fn encoded_flags(root: &Path, target_dir: &Path) -> String {
    let mut flags: Vec<String> = match std::env::var("CARGO_ENCODED_RUSTFLAGS") {
        Ok(e) if !e.is_empty() => e.split('\u{1f}').map(str::to_string).collect(),
        _ => std::env::var("RUSTFLAGS").map(|r| r.split_whitespace().map(str::to_string).collect()).unwrap_or_default(),
    };
    for (from, to) in remaps(root, target_dir) {
        flags.push(format!("--remap-path-prefix={}={to}", from.display()));
    }
    flags.join("\u{1f}")
}

/// On an MSVC target, the flags with the C runtime linked in statically; any other target's flags as given.
pub fn with_static_runtime(target: &str, flags: String) -> String {
    if !target.ends_with("-msvc") {
        return flags;
    }
    if flags.is_empty() {
        return "-Ctarget-feature=+crt-static".to_string();
    }
    format!("{flags}\u{1f}-Ctarget-feature=+crt-static")
}

/// Build both binaries for `target` from the workspace at `root` and lay out the package folder under `out`.
/// Returns the folder.
pub fn package(root: &Path, target: &str, out: &Path) -> Result<PathBuf, String> {
    let root = &plain(root);
    let version = version(root)?;
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let target_dir = plain(&std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| root.join("target")));
    let built = Command::new(&cargo)
        .args(["build", "--release", "--locked", "--target", target, "-p", "app", "-p", "zikaron-cli"])
        .env_remove("RUSTFLAGS")
        .env("CARGO_ENCODED_RUSTFLAGS", with_static_runtime(target, encoded_flags(root, &target_dir)))
        .current_dir(root)
        .status()
        .map_err(|e| format!("{cargo}: {e}"))?;
    if !built.success() {
        return Err(format!("cargo build for {target} failed"));
    }
    lay_out(root, &target_dir.join(target).join("release"), target, out, &version)
}

/// The package folder `ZIKARON-<version>-windows-x86_64`: the window binary (named `zikaron-desk.exe`), the
/// command line (`zikaron.exe`), the licence, and the third-party notices for `target` from the one generator.
pub fn lay_out(root: &Path, bin: &Path, target: &str, out: &Path, version: &str) -> Result<PathBuf, String> {
    let at = out.join(format!("ZIKARON-{version}-windows-x86_64"));
    std::fs::create_dir_all(&at).map_err(|e| format!("{}: {e}", at.display()))?;
    let copy = |from: PathBuf, to: &str| std::fs::copy(&from, at.join(to)).map(|_| ()).map_err(|e| format!("{}: {e}", from.display()));
    copy(bin.join("app.exe"), "zikaron-desk.exe")?;
    copy(bin.join("zikaron.exe"), "zikaron.exe")?;
    copy(root.join("LICENSE"), "LICENSE.txt")?;
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let fonts: Vec<PathBuf> = FONT_LICENCES.iter().map(|f| root.join(f)).collect();
    let notices = crate::notices::make(&cargo, &root.join("Cargo.toml"), target, &["app", "zikaron-cli"], &fonts)?;
    std::fs::write(at.join("THIRD-PARTY-LICENSES.txt"), notices).map_err(|e| e.to_string())?;
    Ok(at)
}

#[cfg(test)]
mod tests {
    /// The home directory is mapped first and the build directory last (the compiler applies the last match,
    /// so the narrower place wins); the checkout and the build directory are always mapped, `..` steps taken out.
    #[test]
    fn the_narrower_place_is_mapped_later() {
        let (root, target) = (std::path::Path::new("/w/zikaron"), std::path::Path::new("/w/zikaron/target"));
        let m = super::remaps(root, target);
        let names: Vec<&str> = m.iter().map(|(_, to)| *to).collect();
        assert_eq!(names.last(), Some(&"/target"));
        assert!(names.contains(&"/zikaron"));
        if names.contains(&"/home") {
            assert_eq!(names.first(), Some(&"/home"));
        }
        assert!(super::encoded_flags(root, target).contains("--remap-path-prefix=/w/zikaron=/zikaron"));
        assert_eq!(super::plain(std::path::Path::new("/w/zikaron/crates/zikaron-pack/../..")), std::path::Path::new("/w/zikaron"));
    }

    /// The C runtime is linked in on an MSVC target only, after the flags already given.
    #[test]
    fn the_runtime_is_linked_in_on_msvc_only() {
        assert_eq!(super::with_static_runtime("x86_64-pc-windows-msvc", "a\u{1f}b".into()), "a\u{1f}b\u{1f}-Ctarget-feature=+crt-static");
        assert_eq!(super::with_static_runtime("x86_64-pc-windows-msvc", String::new()), "-Ctarget-feature=+crt-static");
        assert_eq!(super::with_static_runtime("x86_64-pc-windows-gnu", "a".into()), "a");
    }
}
