#[allow(dead_code)]
fn macos_sdk_link_arg() -> Option<String> {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return None;
    }
    let min = std::env::var("MACOSX_DEPLOYMENT_TARGET").ok().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| {
        if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("aarch64") { "11.0".to_string() } else { "10.12".to_string() }
    });
    Some(format!("-Wl,-platform_version,macos,{min},26.0"))
}
