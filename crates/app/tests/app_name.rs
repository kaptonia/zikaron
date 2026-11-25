fn packaging(rel: &str) -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging").join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn plist_value(plist: &str, key: &str) -> Option<String> {
    let after = plist.split(&format!("<key>{key}</key>")).nth(1)?;
    let open = after.find("<string>")? + "<string>".len();
    let close = after.find("</string>")?;
    Some(after.get(open..close)?.to_string())
}

#[test]
fn the_name_the_system_shows_is_the_apps_name() {
    let name = app::app_name::APP_NAME;
    let plist = packaging("macos/Info.plist");
    assert_eq!(plist_value(&plist, "CFBundleName").as_deref(), Some(name), "macOS CFBundleName");
    assert_eq!(plist_value(&plist, "CFBundleDisplayName").as_deref(), Some(name), "macOS CFBundleDisplayName");
    let desktop = packaging("linux/zikaron-desk.desktop");
    let entry: Vec<&str> = desktop.lines().filter_map(|l| l.strip_prefix("Name=")).collect();
    assert_eq!(entry, [name], "Linux desktop entry Name");
    assert_eq!(app::lang::t(app::lang::Key::AppName), name, "the window's own name");
}
