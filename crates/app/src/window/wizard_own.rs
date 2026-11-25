use super::*;
use crate::own::OwnStep;
use zikaron_ui::sheetparts as parts;

impl Win {
    pub(super) fn own_wiz_card(&mut self, ui: &mut egui::Ui, st: &mut fields::Stack, o: OwnStep, now: f64) {
        let _ = now;
        match o {
            OwnStep::Network => {
                let picked = self.wiz_network_pick();
                let mut rows: Vec<(String, String)> = crate::deploy::KNOWN
                    .iter()
                    .filter(|d| d.name == crate::deploy::DEFAULT || d.name == picked)
                    .map(|d| (d.name.to_string(), t(d.label).to_string()))
                    .collect();
                rows.push((crate::deploy::CUSTOM.to_string(), t(Key::U3Custom).to_string()));
                zikaron_ui::fields::rows(ui, |ui, f| {
                    for (name, title) in rows {
                        let on = picked == name;
                        let (resp, _) = f.line(ui, None, &title, "", false, false, c(C::Ink), |ui| parts::radio(ui, &format!("wiz-network-{name}"), on));
                        let hit = ui.interact(resp.rect, egui::Id::new(("wiz-network", name.as_str())), egui::Sense::click());
                        if hit.clicked() {
                            self.ux.wiz_network = Some(name);
                        }
                        let _ = hit.on_hover_cursor(egui::CursorIcon::PointingHand);
                    }
                });
            }
            OwnStep::Genesis => {
                match self.marked_title() {
                    Some(title) => paint::text(ui, &title, Type::Body, c(C::Ink2)),
                    None => st.field(ui, t(Key::CfWhat), Some(t(Key::U3Optional)), None, |ui, w| fields::line(ui, &mut self.ux.wiz_statement, t(Key::StatementHint), w, fields::Look::default())),
                };
                zikaron_ui::fields::rows(ui, |ui, f| {
                    if f.go(ui, t(Key::AdoptGroup), "", self.shell.home.is_some(), "").clicked() {
                        self.ux.confirm_adopt = true;
                    }
                });
            }
            OwnStep::Gas => self.gas_step_card(ui),
        }
    }

    pub(super) fn own_wiz_keys(&mut self, ui: &mut egui::Ui, o: OwnStep) -> (Option<Action>, bool) {
        match o {
            OwnStep::Network => {
                if page::Page::new().primary(ui, t(Key::WizardNext)).1.clicked() {
                    return (Some(Action::ChooseNetwork { name: self.wiz_network_pick() }), true);
                }
                (None, false)
            }
            OwnStep::Genesis => {
                if self.marked_title().is_some() {
                    if page::Guide::key(ui, t(Key::DoFetchLedger), true).clicked() {
                        self.ux.wizard = None;
                        self.go(Place::Settings(crate::nav::Section::Data), self.ux.now);
                    }
                    return (None, false);
                }
                if page::Guide::key(ui, t(Key::WizGenesisTitle), self.shell.home.is_some()).clicked() {
                    self.ux.confirm_genesis = true;
                }
                (None, false)
            }
            OwnStep::Gas => (self.gas_step_key(ui), false),
        }
    }

    pub(super) fn own_wiz_next(&self, o: OwnStep) -> Option<Action> {
        (o == OwnStep::Network && self.wiz_network_chosen().as_deref() != Some(self.wiz_network_pick().as_str())).then(|| Action::ChooseNetwork { name: self.wiz_network_pick() })
    }

    pub(super) fn own_wiz_readable(&self, o: OwnStep) -> bool {
        matches!(o, OwnStep::Network | OwnStep::Gas)
    }

    pub(super) fn own_wiz_made(&self, o: OwnStep) -> Option<(Key, String)> {
        match o {
            OwnStep::Genesis => self.shell.home.as_ref().map(|h| (Key::CfWhere, h.root().display().to_string())),
            OwnStep::Network | OwnStep::Gas => None,
        }
    }

    pub(super) fn own_wiz_turn(&mut self, ctx: &egui::Context, step: crate::nav::Step, done: bool, now: f64) -> bool {
        self.gas_step_turn(ctx, step == crate::nav::Step::Gas, done, now)
    }

    pub(super) fn genesis_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if !self.ux.confirm_genesis {
            return;
        }
        let root = self.shell.home.as_ref().map(|h| h.root().display().to_string()).unwrap_or_default();
        let blank = self.ux.wiz_statement.trim().is_empty();
        let said = if blank { t(Key::CfBlank).to_string() } else { self.ux.wiz_statement.clone() };
        let (mut go, mut close) = (false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("create-ledger", tk::SHEET_W),
            self,
            |ui, _me| {
                sheet::title(ui, t(Key::WizGenesisTitle), "");
                fields::stack(ui, |ui, _st| {
                    let copied = zikaron_ui::fields::rows(ui, |ui, f| {
                        let _ = f.info(ui, t(Key::CfWhat), if blank { card::Show::Empty(&said) } else { card::Show::Text(&said) });
                        let _ = f.info(ui, t(Key::CfCost), card::Show::Text(t(Key::CfCostLocal)));
                        f.info(ui, t(Key::CfWhere), if root.is_empty() { card::Show::Empty(t(Key::PickNone)) } else { card::Show::Copy(&root) })
                    });
                    tell_copied(ui.ctx(), copied);
                });
            },
            |ui, _me| {
                go = page::Pen::new().press(ui, t(Key::WizGenesisTitle), true).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
            },
        );
        if close || out.esc {
            self.ux.confirm_genesis = false;
        } else if go {
            self.ux.confirm_genesis = false;
            self.act(Action::Genesis { statement: self.ux.wiz_statement.clone() }, now);
        }
    }

    pub(super) fn wizard_adopt_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if !self.ux.confirm_adopt {
            return;
        }
        if self.ux.wiz_adopt.trim().is_empty() {
            self.ux.wiz_adopt = self.shell.documents.clone();
        }
        let (mut go, mut close, mut picked) = (false, false, None);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("wizard-adopt", tk::SHEET_W),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::AdoptGroup), "");
                zikaron_ui::fields::rows(ui, |ui, f| f.folder(ui, t(Key::WizAdoptFolder), &me.ux.wiz_adopt, t(Key::PickNone)));
            },
            |ui, me| {
                go = page::Pen::new().press(ui, t(Key::DoAdopt), !me.ux.wiz_adopt.trim().is_empty()).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                picked = foot_pick(ui, "wizard-adopt", crate::platform::Pick::Folder);
            },
        );
        if let Some(p) = picked {
            self.ux.wiz_adopt = p;
        }
        if close || out.esc {
            self.ux.confirm_adopt = false;
        } else if go {
            self.ux.confirm_adopt = false;
            self.act(Action::Adopt { dir: self.ux.wiz_adopt.trim().to_string() }, now);
        }
    }
}
