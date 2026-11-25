use super::probe_shell;

#[test]
fn a_passcode_part_filled_moves_on_by_itself() {
    if super::alone_in(module_path!(), "a_passcode_part_filled_moves_on_by_itself") {
        return;
    }
    let ctx = zikaron_ui::egui::Context::default();
    let shell = probe_shell(&ctx);
    let (_, (at_new, at_again)) = app::window::probe_pin_moves_on(&ctx, shell, "A1b2C3d4", "Q7w8E9r0", 1180.0, 800.0);
    assert!(at_new, "the old passcode whole: the sheet stands at the new one by itself");
    assert!(at_again, "the new one whole: the sheet stands at \"again\" by itself");
}

#[test]
fn an_identity_is_named_in_one_place_everywhere() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/window");
    let mut got: Vec<(String, usize)> = Vec::new();
    for e in std::fs::read_dir(&dir).expect("window").flatten() {
        let p = e.path();
        let text = std::fs::read_to_string(&p).expect("a source file");
        let n = super::code_only(&text).matches("t(Key::Unnamed)").count();
        if n > 0 {
            got.push((p.file_name().unwrap().to_string_lossy().to_string(), n));
        }
    }
    got.sort();
    assert_eq!(got, vec![("chrome.rs".to_string(), 1), ("identity.rs".to_string(), 2), ("mod.rs".to_string(), 2), ("persona.rs".to_string(), 1)], "where \"unnamed\" is written");
    let chrome = super::code_only(&std::fs::read_to_string(dir.join("chrome.rs")).expect("chrome"));
    assert_eq!(chrome.matches("super::identity::id_name(").count(), 2, "the chip and its menu");
}

#[test]
fn a_setting_rows_typing_belongs_to_its_home() {
    if super::alone_in(module_path!(), "a_setting_rows_typing_belongs_to_its_home") {
        return;
    }
    let ctx = zikaron_ui::egui::Context::default();
    let shell = super::probe_shell(&ctx);
    let (_, (stood, gone)) = app::window::probe_setting_rows_follow(&ctx, shell, 1180.0, 800.0);
    assert!(stood, "on the same seat the typing and \"saved\" stand");
    assert!(gone, "on the other seat both are gone");
}
