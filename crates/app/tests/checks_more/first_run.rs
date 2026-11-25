use app::nav::{Progress, Step};
use app::own::{OwnProgress, OwnStep};
use app::roles::Role;

#[test]
fn a_step_of_the_products_own_is_one_variant_said_by_its_own_name() {
    let names: Vec<String> = Step::ALL.iter().map(|s| format!("{s:?}")).collect();
    assert_eq!(names, ["Pin", "Key", "Network", "Genesis", "Gas", "Backup"]);
    assert_eq!(Step::Network, Step::Own(OwnStep::Network));
    let all = Progress { key: true, pin: true, backup: true, own: OwnProgress { network: true, gas: true, genesis: true } };
    for role in Role::ALL {
        assert!(!all.wants_wizard(role));
        assert!(Progress { pin: false, ..all }.wants_wizard(role));
        assert!(Progress { key: false, ..all }.wants_wizard(role));
        assert!(!Progress { backup: false, own: OwnProgress { network: false, gas: false, genesis: false }, ..all }.wants_wizard(role), "{role:?}");
    }
}

#[test]
fn the_column_of_steps_is_measured_once_and_kept() {
    if super::alone_in(module_path!(), "the_column_of_steps_is_measured_once_and_kept") {
        return;
    }
    let ctx = zikaron_ui::egui::Context::default();
    let shell = super::probe_shell(&ctx);
    let _ = app::window::probe_wizard_step(&ctx, shell, app::nav::Place::Home, Step::Pin, 1180.0, 800.0);
    let ppp = ctx.pixels_per_point().to_bits();
    let kept = ctx.data(|d| d.get_temp::<f32>(zikaron_ui::egui::Id::new(("zikaron-wizard-steps-w", ppp))));
    assert!(kept.is_some_and(|w| w > 0.0), "the width kept after the first frame: {kept:?}");
}
