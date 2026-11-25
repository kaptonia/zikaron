use std::cell::Cell;
use zikaron_ui::egui::{self, pos2, vec2, Event, Pos2, Rect};
use zikaron_ui::{button, fields, layer, sheet, tokens};

fn central<R>(ctx: &egui::Context, panel: egui::CentralPanel, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    let mut top = egui::Ui::new(ctx.clone(), egui::Id::new((ctx.viewport_id(), "central_panel")), egui::UiBuilder::new().layer_id(egui::LayerId::background()).max_rect(ctx.content_rect()));
    top.set_clip_rect(ctx.content_rect());
    panel.show_inside(&mut top, add)
}

const SCREEN: egui::Vec2 = egui::Vec2 { x: 1180.0, y: 800.0 };

struct Frames {
    ctx: egui::Context,
    t: f64,
}

impl Frames {
    fn new() -> Frames {
        let ctx = egui::Context::default();
        let _ = zikaron_ui::skin::dress(&ctx);
        Frames { ctx, t: 0.0 }
    }

    fn run(&mut self, dt: f64, events: Vec<Event>, draw: bool, mut ui: impl FnMut(&egui::Context)) -> egui::FullOutput {
        self.t += dt;
        let mut input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), time: Some(self.t), events, ..Default::default() };
        input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(2.0);
        self.ctx.run_ui(input, |root| {
            let ctx = &root.ctx().clone();
            if draw {
                ui(ctx);
                layer::fade_out(ctx);
            }
        })
    }
}

fn card_layer(name: &str) -> egui::LayerId {
    egui::LayerId::new(egui::Order::Foreground, egui::Id::new(("zikaron-sheet", name)).with("card"))
}

fn texts(out: &egui::FullOutput) -> Vec<(String, Rect)> {
    out.shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::Shape::Text(t) => Some((t.galley.text().to_string(), t.galley.rect.translate(t.pos.to_vec2()))),
            _ => None,
        })
        .collect()
}

fn shapes_in(out: &egui::FullOutput, strip: Rect) -> usize {
    out.shapes.iter().filter(|cs| cs.shape.visual_bounding_rect().is_positive() && strip.contains_rect(cs.shape.visual_bounding_rect())).count()
}

#[test]
fn a_scroll_bar_shows_only_when_the_body_does_not_fit() {
    for (tall, name, bar) in [(200.3_f32, "fits", false), (2000.0, "long", true)] {
        let mut f = Frames::new();
        let mut n = 0;
        let card = Cell::new(Rect::NOTHING);
        let draw = |ctx: &egui::Context, n: &mut i32| {
            sheet::show(
                ctx,
                sheet::Spec::new(name, tokens::SHEET_W),
                n,
                |ui, _| {
                    let r = ui.allocate_exact_size(vec2(ui.available_width(), tall), egui::Sense::hover()).0;
                    card.set(r);
                },
                |ui, _| {
                    let _ = button::key(ui, "取消", button::Role::Secondary, true);
                },
            );
        };
        for _ in 0..30 {
            f.run(0.05, vec![], true, |ctx| draw(ctx, &mut n));
        }
        let body = card.get();
        let over = vec![Event::PointerMoved(pos2(body.right() + 16.0, body.top() + 40.0))];
        let mut out = f.run(0.05, over, true, |ctx| draw(ctx, &mut n));
        for _ in 0..10 {
            out = f.run(0.05, vec![], true, |ctx| draw(ctx, &mut n));
        }
        let strip = Rect::from_min_max(pos2(body.right() + 4.0, body.top() - 24.0), pos2(body.right() + 28.0, body.top() + 400.0));
        let drawn = shapes_in(&out, strip);
        assert_eq!(drawn > 0, bar, "{name}: shapes along the right edge {drawn}");
    }
}

#[test]
fn a_fold_row_ending_the_body_stands_off_the_keys_as_a_field_does() {
    let mut gaps: Vec<(String, f32)> = Vec::new();
    for form in ["field", "closed", "open"] {
        let mut f = Frames::new();
        let shown = Cell::new(Rect::NOTHING);
        let mut s = String::new();
        let name = format!("fold-{form}");
        let draw = |ctx: &egui::Context, s: &mut String| {
            sheet::show(
                ctx,
                sheet::Spec::new(&name, tokens::SHEET_W),
                s,
                |ui, s| {
                    fields::stack(ui, |ui, st| {
                        st.field(ui, "名称", None, None, |ui, w| {
                            let _ = fields::line(ui, s, "", w, fields::Look::default());
                        });
                        if form == "field" {
                            st.field(ui, "另一格", None, None, |ui, w| shown.set(ui.allocate_exact_size(vec2(w, tokens::INPUT_H), egui::Sense::hover()).0));
                        }
                    });
                    if form != "field" {
                        fields::rows(ui, |ui, f| {
                            f.fold_from(ui, "more", "高级选项", "", form == "open", |ui, f| {
                                f.panel(ui, |ui| {
                                    let r = ui.allocate_exact_size(vec2(ui.available_width(), tokens::INPUT_H), egui::Sense::hover()).0;
                                    shown.set(r);
                                });
                            });
                        });
                    }
                },
                |ui, _| {
                    let _ = button::key(ui, "确认", button::Role::Primary, true);
                },
            );
        };
        let mut out = f.run(0.05, vec![], true, |ctx| draw(ctx, &mut s));
        for _ in 0..40 {
            out = f.run(0.05, vec![], true, |ctx| draw(ctx, &mut s));
        }
        let t = texts(&out);
        let key = t.iter().find(|(w, _)| w == "确认").map(|(_, r)| r.center().y - tokens::KEY_H / 2.0).expect("the key");
        let bottom = if form == "closed" {
            t.iter().find(|(w, _)| w == "高级选项").map(|(_, r)| r.center().y + 21.0 / 2.0).expect("the fold's words")
        } else {
            shown.get().bottom()
        };
        gaps.push((form.to_string(), key - bottom));
    }
    let field = gaps[0].1;
    for (form, g) in &gaps {
        assert!((g - field).abs() <= 1.0, "{form}: {g} off the keys, a field {field} ({gaps:?})");
    }
}

#[test]
fn a_pass_drawn_by_nobody_does_not_bring_the_sheet_again() {
    let mut f = Frames::new();
    let mut n = 0;
    let draw = |ctx: &egui::Context, n: &mut i32| {
        sheet::show(ctx, sheet::Spec::new("covered", tokens::SHEET_W), n, |ui, _| ui.label("卡里"), |_, _| ());
    };
    for _ in 0..40 {
        f.run(0.05, vec![], true, |ctx| draw(ctx, &mut n));
    }
    let scale = |f: &Frames| f.ctx.layer_transform_to_global(card_layer("covered")).map(|t| t.scaling).unwrap_or(1.0);
    assert_eq!(scale(&f), 1.0, "whole before");
    for _ in 0..5 {
        f.run(0.5, vec![], false, |_| ());
    }
    let out = f.run(0.016, vec![], true, |ctx| draw(ctx, &mut n));
    assert_eq!(scale(&f), 1.0, "whole the first frame it shows again");
    let words = texts(&out);
    assert!(words.iter().any(|(w, _)| w == "卡里"), "drawn");
    let leaving = egui::LayerId::new(egui::Order::Foreground, card_layer("covered").id.with("leaving"));
    assert!(out.shapes.iter().all(|cs| f.ctx.layer_id_at(cs.clip_rect.center()) != Some(leaving)), "no leaving copy");
}

#[test]
fn a_fold_row_ending_a_page_form_card_stands_off_its_keys_as_a_field_does() {
    let mut gaps: Vec<(String, f32)> = Vec::new();
    for form in ["field", "closed", "open"] {
        let mut f = Frames::new();
        let shown = Cell::new(Rect::NOTHING);
        let mut s = String::new();
        let draw = |ctx: &egui::Context, s: &mut String| {
            central(ctx, egui::CentralPanel::default(), |ui| {
                fields::card(ui, |ui, st| {
                    st.field(ui, "名称", None, None, |ui, w| {
                        let _ = fields::line(ui, s, "", w, fields::Look::default());
                    });
                    if form == "field" {
                        st.field(ui, "另一格", None, None, |ui, w| shown.set(ui.allocate_exact_size(vec2(w, tokens::INPUT_H), egui::Sense::hover()).0));
                    } else {
                        fields::rows(ui, |ui, f| {
                            f.fold_from(ui, "page-more", "高级选项", "", form == "open", |ui, f| {
                                f.panel(ui, |ui| {
                                    let r = ui.allocate_exact_size(vec2(ui.available_width(), tokens::INPUT_H), egui::Sense::hover()).0;
                                    shown.set(r);
                                });
                            });
                        });
                    }
                    st.foot(ui, |ui| {
                        let _ = button::key(ui, "核验", button::Role::Primary, true);
                    });
                });
            });
        };
        let mut out = f.run(0.05, vec![], true, |ctx| draw(ctx, &mut s));
        for _ in 0..40 {
            out = f.run(0.05, vec![], true, |ctx| draw(ctx, &mut s));
        }
        let t = texts(&out);
        let key = t.iter().find(|(w, _)| w == "核验").map(|(_, r)| r.center().y - tokens::KEY_H / 2.0).expect("the key");
        let bottom = if form == "closed" {
            t.iter().find(|(w, _)| w == "高级选项").map(|(_, r)| r.center().y + 21.0 / 2.0).expect("the fold's words")
        } else {
            shown.get().bottom()
        };
        gaps.push((form.to_string(), key - bottom));
    }
    let field = gaps[0].1;
    for (form, g) in &gaps {
        assert!((g - field).abs() <= 1.0, "{form}: {g} off the keys, a field {field} ({gaps:?})");
    }
}

#[test]
fn a_foot_laid_past_its_row_is_said() {
    use zikaron_ui::button::Role;
    let mut f = Frames::new();
    let mut n = 0;
    for _ in 0..20 {
        f.run(0.05, vec![], true, |ctx| {
            sheet::show(ctx, sheet::Spec::new("foot-keys", tokens::SHEET_W), &mut n, |ui, _| ui.label("卡里"), |ui, _| {
                let _ = button::key(ui, "继续", Role::Primary, true);
                let _ = button::key(ui, "返回", Role::Secondary, true);
            });
        });
    }
    assert!(!zikaron_ui::probe::feet_out().iter().any(|o| o.id == "foot-keys"), "two keys at the right: {:?}", zikaron_ui::probe::feet_out());
    for _ in 0..20 {
        f.run(0.05, vec![], true, |ctx| {
            sheet::show(ctx, sheet::Spec::new("foot-note", tokens::SHEET_W), &mut n, |ui, _| ui.label("卡里"), |ui, _| {
                zikaron_ui::states::note_box(ui, "一段说明,画在了卡底那一行钮里,占满整行,其后的钮被排到卡外。");
                let _ = button::key(ui, "撤回", Role::Primary, true);
                let _ = button::key(ui, "返回", Role::Secondary, true);
            });
        });
    }
    let out = zikaron_ui::probe::feet_out();
    let o = out.iter().find(|o| o.id == "foot-note").unwrap_or_else(|| panic!("a note in the row of keys is said: {out:?}"));
    assert!(zikaron_ui::probe::overruns(o.row, o.laid), "{o:?}");
}

#[test]
fn a_sheet_widens_to_the_keys_it_lays() {
    use zikaron_ui::button::Role;
    let mut f = Frames::new();
    let mut n = 0;
    for _ in 0..30 {
        f.run(0.05, vec![], true, |ctx| {
            sheet::show(ctx, sheet::Spec::new("wide-keys", tokens::SHEET_W), &mut n, |ui, _| ui.label("卡里"), |ui, _| {
                let _ = button::key(ui, "Go put them on chain, all of them now", Role::Primary, true);
                let _ = button::key(ui, "Cancel this and come back later", Role::Secondary, true);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| button::key(ui, "Cancel key change", Role::Secondary, true));
            });
        });
    }
    let out = zikaron_ui::probe::feet_out();
    assert!(!out.iter().any(|o| o.id == "wide-keys"), "keys side by side within the card: {out:?}");
    let card = zikaron_ui::probe::sheets(&f.ctx).into_iter().filter(|s| s.id == "wide-keys").map(|s| s.card).last().expect("drawn");
    assert!(card.width() > tokens::SHEET_W, "widened: {card:?}");
}
