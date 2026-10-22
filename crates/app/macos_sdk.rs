// The window's design is macOS 26's: on macOS 26 and later the system draws a window the way it draws the
// windows of apps built for it (Tahoe's corners, concentric with the window's controls) only for a program
// that records it was built for SDK 26. This build records that, whatever SDK the machine that builds it has;
// the oldest macOS it runs on stays the target's own. Included by the app's and the door's build scripts, the
// one place it is said.

/// The linker argument that records SDK 26 for a macOS target, or none for any other target.
#[allow(dead_code)]
fn macos_sdk_link_arg() -> Option<String> {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return None;
    }
    // The oldest macOS the build runs on: the deployment target when one is set, else rustc's own for the
    // architecture.
    let min = std::env::var("MACOSX_DEPLOYMENT_TARGET").ok().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| {
        if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("aarch64") { "11.0".to_string() } else { "10.12".to_string() }
    });
    Some(format!("-Wl,-platform_version,macos,{min},26.0"))
}
