#[test]
fn a_records_file_names_follow_the_table() {
    let ctx = zikaron_ui::egui::Context::default();
    let shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let ((stale, asked), (current, asked_again)) = app::window::probe_files_follow_the_table(shell);
    assert!(stale == vec!["old.txt".to_string()] || stale.is_empty(), "the names held until the new land, or what the index holds now (none here): {stale:?}");
    assert!(asked, "the index asked again for the new generation");
    assert_eq!(current, vec!["new.txt".to_string()]);
    assert!(!asked_again, "nothing asked for the generation held");
}
