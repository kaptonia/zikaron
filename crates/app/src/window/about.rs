use super::*;
use crate::platform::menubar::{Act, Column, Words};

pub(super) fn bar_words() -> Words {
    let name = t(Key::AppName);
    let item = |a: Act| -> String {
        match a {
            Act::About => fill1(Key::MenuAbout, name),
            Act::Settings => t(Key::MenuSettings).to_string(),
            Act::Services => t(Key::MenuServices).to_string(),
            Act::Hide => fill1(Key::MenuHide, name),
            Act::HideOthers => t(Key::MenuHideOthers).to_string(),
            Act::ShowAll => t(Key::MenuShowAll).to_string(),
            Act::Quit => fill1(Key::MenuQuit, name),
            Act::Cut => t(Key::MenuCut).to_string(),
            Act::Copy => t(Key::MenuCopy).to_string(),
            Act::Paste => t(Key::MenuPaste).to_string(),
            Act::SelectAll => t(Key::MenuSelectAll).to_string(),
            Act::Minimize => t(Key::MenuMinimize).to_string(),
            Act::Zoom => t(Key::MenuZoom).to_string(),
            Act::Close => t(Key::ToastClose).to_string(),
        }
    };
    let column = |c: Column| -> String {
        match c {
            Column::App => name.to_string(),
            Column::Edit => t(Key::DoEdit).to_string(),
            Column::Window => t(Key::MenuWindow).to_string(),
        }
    };
    Words {
        columns: [Column::App, Column::Edit, Column::Window].into_iter().map(|c| (c, column(c))).collect(),
        items: crate::platform::menubar::acts().into_iter().map(|a| (a, item(a))).collect(),
    }
}

impl Win {
    pub(super) fn rail_keys_live(&self, ctx: &egui::Context) -> bool {
        self.rail_shown() && !sheet::up(ctx) && !self.any_sheet()
    }

    pub(super) fn bar_turn(&mut self, ctx: &egui::Context, now: f64) {
        crate::platform::menu_bar(&bar_words());
        let live = self.rail_keys_live(ctx);
        crate::platform::menubar::set_settings_reachable(live);
        for a in crate::platform::menubar::asked() {
            match a {
                Act::Settings if live => self.rail_to(self.settings_place(), now),
                Act::About => self.ux.about_open = true,
                _ => {}
            }
        }
    }

    pub(super) fn about_window(&mut self, ctx: &egui::Context) {
        if !self.ux.about_open {
            return;
        }
        let facts = crate::about::Facts::now();
        let icon = self
            .ux
            .about_icon
            .get_or_insert_with(|| ctx.load_texture("zikaron-about-icon", egui::ColorImage::from_rgba_unmultiplied([facts.icon_side as usize; 2], facts.icon), egui::TextureOptions::LINEAR))
            .clone();
        let mut closed = false;
        let title = fill1(Key::MenuAbout, &facts.name);
        let builder = egui::ViewportBuilder::default().with_title(title).with_inner_size([300.0, 260.0]).with_resizable(false).with_minimize_button(false).with_maximize_button(false);
        ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("zikaron-about"), builder, |root, _| {
            egui::CentralPanel::default().show_inside(root, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(24.0);
                    ui.add(egui::Image::new(&icon).fit_to_exact_size(egui::vec2(96.0, 96.0)));
                    ui.add_space(14.0);
                    ui.label(egui::RichText::new(&facts.name).font(zikaron_ui::fonts::about_name()).color(zikaron_ui::palette::c(zikaron_ui::palette::C::Ink)));
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(&facts.version).font(tk::Type::Small.font()).color(zikaron_ui::palette::c(zikaron_ui::palette::C::Ink2)));
                    if let Some(u) = &facts.updated {
                        ui.label(egui::RichText::new(u).font(tk::Type::Small.font()).color(zikaron_ui::palette::c(zikaron_ui::palette::C::Ink2)));
                    }
                });
            });
            if root.ctx().input(|i| i.viewport().close_requested()) {
                closed = true;
            }
        });
        if closed {
            self.ux.about_open = false;
        }
    }
}
