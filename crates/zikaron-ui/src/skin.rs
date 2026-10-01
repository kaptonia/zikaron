//! Skin: install the settled numbers and colors into egui once, and follow the appearance every frame.
//!
//! After `dress` no color, corner or font size needs writing again. The appearance (light, dark, or the
//! system's) blends the two color tables over `tokens::SLOW`; while it moves, the colors egui itself uses
//! (panel fill, selection, scroll bars) are installed again each frame.

use crate::palette::{self, c, C};
use crate::tokens::{self, Radius, Type};

/// The three appearances. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Appearance {
    #[default]
    Light,
    Dark,
    System,
}

impl Appearance {
    pub const ALL: [Appearance; 3] = [Appearance::Light, Appearance::Dark, Appearance::System];

    /// The name written to disk.
    pub fn as_str(self) -> &'static str {
        match self {
            Appearance::Light => "light",
            Appearance::Dark => "dark",
            Appearance::System => "system",
        }
    }

    pub fn named(s: &str) -> Option<Appearance> {
        Appearance::ALL.iter().copied().find(|a| a.as_str() == s)
    }
}

/// Install everything once: sizes, spacing and the colors of the current blend.
pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    install_colors(&mut style.visuals);
    use egui::{FontFamily as F, FontId, TextStyle as T};
    style.text_styles = [
        (T::Heading, Type::Page.font()),
        (T::Body, Type::Body.font()),
        (T::Button, Type::Key.font()),
        (T::Small, Type::Small.font()),
        (T::Monospace, FontId::new(Type::Mono.size(), F::Monospace)),
    ]
    .into();
    // A row is as tall as what it holds: text keeps its line height, keys and fields size themselves.
    style.spacing.interact_size = egui::vec2(tokens::KEY_H * 2.0, 0.0);
    style.spacing.item_spacing = egui::vec2(tokens::S2, tokens::S3);
    style.spacing.button_padding = egui::vec2(tokens::KEY_PAD_X, 8.0);
    style.spacing.window_margin = egui::Margin::same(tokens::SHEET_PAD as i8);
    style.spacing.indent = tokens::S3;
    style.spacing.menu_margin = egui::Margin::same(5);
    let mut scroll = egui::style::ScrollStyle::thin();
    scroll.bar_width = 10.0;
    scroll.floating_width = 6.0;
    // Floating bars lie over the content and take no width: a bar appearing when the content just overflows
    // must not narrow it (a narrower page can lay out taller or shorter, the bar goes, the page widens again,
    // and a width-chosen layout flips each frame).
    scroll.floating_allocated_width = 0.0;
    scroll.handle_min_length = 28.0;
    scroll.bar_inner_margin = 2.0;
    scroll.bar_outer_margin = 0.0;
    scroll.foreground_color = false;
    scroll.dormant_background_opacity = 0.0;
    scroll.active_background_opacity = 0.0;
    scroll.interact_background_opacity = 0.0;
    scroll.dormant_handle_opacity = 0.0;
    scroll.active_handle_opacity = 1.0;
    scroll.interact_handle_opacity = 1.0;
    style.spacing.scroll = scroll;
    style.animation_time = tokens::MID;
    ctx.set_style_of(egui::Theme::Light, style.clone());
    ctx.set_style_of(egui::Theme::Dark, style);
    ctx.set_theme(egui::Theme::Light);
}

fn install_colors(v: &mut egui::Visuals) {
    v.dark_mode = palette::blend() > 0.5;
    v.panel_fill = c(C::Ground);
    v.window_fill = c(C::Surface);
    v.extreme_bg_color = c(C::Surface);
    v.faint_bg_color = c(C::Hover);
    v.code_bg_color = c(C::Rail);
    v.override_text_color = None;
    v.hyperlink_color = c(C::AccentInk);
    v.warn_fg_color = c(C::Warn);
    v.error_fg_color = c(C::Bad);
    v.window_stroke = egui::Stroke::new(1.0_f32, c(C::Line));
    v.window_shadow = egui::Shadow::NONE;
    v.popup_shadow = egui::Shadow::NONE;
    v.selection.bg_fill = c(C::AccentWash);
    v.selection.stroke = egui::Stroke::new(1.0_f32, c(C::AccentInk));
    v.text_cursor.stroke = egui::Stroke::new(1.5_f32, c(C::Accent));
    v.window_corner_radius = Radius::Sheet.egui();
    v.menu_corner_radius = Radius::Menu.egui();
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = Radius::Ctl.egui();
        w.bg_stroke = egui::Stroke::new(1.0_f32, c(C::Line));
        w.fg_stroke = egui::Stroke::new(1.0_f32, c(C::Ink));
        w.expansion = 0.0;
        w.weak_bg_fill = c(C::Surface);
    }
    // Scroll handles take these grounds: a light line color, darker when held.
    v.widgets.inactive.bg_fill = c(C::Line);
    v.widgets.hovered.bg_fill = c(C::Line);
    v.widgets.active.bg_fill = c(C::Dash);
    v.widgets.noninteractive.bg_fill = c(C::Surface);
    v.widgets.open.bg_fill = c(C::Hover);
}

/// Follow the appearance this frame: ease the blend toward light (0) or dark (1) over 300 ms and install
/// the blended colors while it moves. Returns the blend.
pub fn follow(ctx: &egui::Context, a: Appearance) -> f32 {
    let dark = match a {
        Appearance::Light => false,
        Appearance::Dark => true,
        Appearance::System => ctx.system_theme() == Some(egui::Theme::Dark),
    };
    let before = palette::blend();
    let b = crate::motion::to(ctx, egui::Id::new("zikaron-appearance"), if dark { 1.0 } else { 0.0 }, tokens::SLOW, crate::motion::Curve::Ease);
    palette::set_blend(b);
    if (b - before).abs() > f32::EPSILON {
        let mut style = (*ctx.style()).clone();
        install_colors(&mut style.visuals);
        ctx.set_style_of(egui::Theme::Light, style.clone());
        ctx.set_style_of(egui::Theme::Dark, style);
    }
    b
}

/// The result of dressing: the found fonts and the roles not found.
pub struct Dressed {
    pub found: crate::fonts::Found,
    pub missing: Vec<crate::fonts::Role>,
}

/// Dress: install skin and fonts at once. The window and the test hooks both call it.
pub fn dress(ctx: &egui::Context) -> Dressed {
    apply(ctx);
    let found = crate::fonts::find();
    let mut missing = crate::fonts::install(ctx, &found);
    missing.sort_by_key(|r| r.as_str());
    missing.dedup();
    Dressed { found, missing }
}

/// Numbers read back from the installed style (for on-screen diagnostics and measurements), read from egui's
/// own state, not from the constants just written.
#[derive(Clone, Debug, PartialEq)]
pub struct Installed {
    pub body: f32,
    pub small: f32,
    pub heading: f32,
    pub mono: f32,
    pub control_radius: u8,
    pub panel: [u8; 4],
}

pub fn installed(ctx: &egui::Context) -> Installed {
    let s = ctx.style();
    let size = |t: egui::TextStyle| s.text_styles.get(&t).map(|f| f.size).unwrap_or(0.0);
    let body = size(egui::TextStyle::Body);
    let small = size(egui::TextStyle::Small);
    let heading = size(egui::TextStyle::Heading);
    let mono = size(egui::TextStyle::Monospace);
    Installed {
        body,
        small,
        heading,
        mono,
        control_radius: s.visuals.widgets.inactive.corner_radius.nw,
        panel: s.visuals.panel_fill.to_array(),
    }
}
