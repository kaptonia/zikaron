const POINTS: &[(&str, &str, &str, &str)] = &[
    ("window/identity.rs", "own::IMPORT_TABS", "window/own.rs", "pub(super) const IMPORT_TABS"),
    ("window/identity.rs", "own::IMPORT_SEAT_ROW", "window/own.rs", "pub(super) const IMPORT_SEAT_ROW"),
    ("window/identity.rs", "self.own_gas_fact()", "window/own.rs", "fn own_gas_fact("),
    ("window/identity.rs", "self.own_primary_how(", "window/own.rs", "fn own_primary_how("),
    ("window/identity.rs", "self.own_keys_after(", "window/own.rs", "fn own_keys_after("),
    ("window/identity.rs", "own_network_row(", "window/own.rs", "fn own_network_row("),
    ("window/identity.rs", "self.own_card_more(", "window/own.rs", "fn own_card_more("),
    ("window/identity.rs", "self.own_id_rows(", "window/own.rs", "fn own_id_rows("),
    ("window/identity.rs", "self.own_key_rows(", "window/own.rs", "fn own_key_rows("),
    ("window/identity.rs", "self.own_details(", "window/own.rs", "fn own_details("),
    ("window/identity.rs", "own_delete_effect(", "window/own.rs", "fn own_delete_effect("),
    ("window/identity.rs", "self.own_hold(", "window/own.rs", "fn own_hold("),
    ("window/backup.rs", "self.own_bk_sheet(", "window/own.rs", "fn own_bk_sheet("),
    ("window/backup.rs", "self.own_bk_done(", "window/own.rs", "fn own_bk_done("),
    ("window/backup.rs", "self.own_backup_rows(", "window/own.rs", "fn own_backup_rows("),
    ("window/backup.rs", "self.own_backup_after(", "window/own.rs", "fn own_backup_after("),
    ("window/backup.rs", "self.own_hold(", "window/own.rs", "fn own_hold("),
    ("window/wizard.rs", "self.own_wiz_card(", "window/wizard_own.rs", "fn own_wiz_card("),
    ("window/wizard.rs", "self.own_wiz_keys(", "window/wizard_own.rs", "fn own_wiz_keys("),
    ("window/wizard.rs", "self.own_wiz_next(", "window/wizard_own.rs", "fn own_wiz_next("),
    ("window/wizard.rs", "self.own_wiz_readable(", "window/wizard_own.rs", "fn own_wiz_readable("),
    ("window/wizard.rs", "self.own_wiz_made(", "window/wizard_own.rs", "fn own_wiz_made("),
    ("window/wizard.rs", "self.own_wiz_turn(", "window/wizard_own.rs", "fn own_wiz_turn("),
    ("window/wizard.rs", "self.own_rail_extra(", "window/own.rs", "fn own_rail_extra("),
    ("window/wizard.rs", "self.own_rail_go(", "window/own.rs", "fn own_rail_go("),
    ("window/wizard.rs", "self.own_finished_words()", "window/own.rs", "fn own_finished_words("),
    ("window/modals.rs", "self.own_wiz_enter(", "window/own.rs", "fn own_wiz_enter("),
    ("steps.rs", "crate::own::WALK", "own.rs", "pub const WALK"),
    ("window/drive.rs", "own::APP_ID", "window/own.rs", "pub(super) const APP_ID"),
    ("window/drive.rs", "own::icon()", "window/own.rs", "fn icon("),
    ("window/drive.rs", "own::MENU_BAR", "window/own.rs", "pub(super) const MENU_BAR"),
    ("window/drive.rs", "own::GATE_KEY", "window/own.rs", "pub(super) const GATE_KEY"),
    ("window/drive.rs", "own_start(", "window/own.rs", "fn own_start("),
    ("window/drive.rs", "own_stage(", "window/own.rs", "fn own_stage("),
    ("window/skeleton.rs", "self.own_pushed(", "window/own.rs", "fn own_pushed("),
    ("window/skeleton.rs", "self.body_before(", "window/pages.rs", "fn body_before("),
    ("window/skeleton.rs", "self.follow_extra(", "window/pages.rs", "fn follow_extra("),
    ("window/network.rs", "self.own_net_task()", "window/own.rs", "fn own_net_task("),
    ("window/network.rs", "own_net_key(", "window/own.rs", "fn own_net_key("),
    ("window/network.rs", "self.own_net_go(", "window/own.rs", "fn own_net_go("),
    ("window/network.rs", "self.own_net_overview(", "window/own.rs", "fn own_net_overview("),
    ("window/network.rs", "self.own_after_rest(", "window/own.rs", "fn own_after_rest("),
    ("window/network.rs", "self.own_net_details()", "window/own.rs", "fn own_net_details("),
    ("window/network.rs", "self.own_home_chain()", "window/own.rs", "fn own_home_chain("),
    ("deploy.rs", "crate::own::DEPLOYMENTS", "own.rs", "pub const DEPLOYMENTS"),
    ("deploy.rs", "crate::own::DEPLOY_DEFAULT", "own.rs", "pub const DEPLOY_DEFAULT"),
    ("deploy.rs", "crate::own::DEPLOY_CODE_HASH", "own.rs", "pub const DEPLOY_CODE_HASH"),
    ("deploy.rs", "crate::own::DeployOwn", "own.rs", "pub type DeployOwn"),
];

#[test]
fn every_registration_point_is_called_by_its_component_and_answered_by_the_product() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let read = |f: &str| super::code_only(&std::fs::read_to_string(src.join(f)).unwrap_or_else(|_| panic!("{f} reads")));
    for (component, call, product, answer) in POINTS {
        assert!(read(component).contains(call), "{component} calls {call}");
        assert_eq!(read(product).matches(answer).count(), 1, "{product} answers {answer} once");
    }
}
