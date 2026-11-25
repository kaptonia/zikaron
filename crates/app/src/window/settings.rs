use super::*;

impl Win {
    pub(super) fn settings_home(&mut self, ui: &mut egui::Ui, now: f64) {
        let role = self.shell.settings.role;
        let mut open: Option<Section> = None;
        stagger(ui, 0, |ui| {
            card::form(ui, |ui, f| {
                for s in crate::nav::sections(role) {
                    if f.go_glyph(ui, s.glyph(), t(s.key())).clicked() {
                        open = Some(s);
                    }
                }
            });
        });
        if let Some(s) = open {
            self.push(Route::Section(s), now);
        }
    }

    pub(super) fn settings_section(&mut self, ui: &mut egui::Ui, s: Section, now: f64) {
        match s {
            Section::Language => self.set_language(ui, now),
            Section::Appearance => self.set_appearance(ui, now),
            Section::Keys => self.set_keys(ui, now),
            Section::Network => self.set_network(ui, now),
            Section::Notify => self.set_notify(ui, now),
            Section::Data => self.set_data(ui, now),
            Section::Cli => self.set_cli(ui, now),
            Section::About => self.set_about(ui, now),
        }
    }

    fn set_language(&mut self, ui: &mut egui::Ui, now: f64) {
        let langs = crate::lang::Lang::ALL;
        let cur = langs.iter().position(|l| *l == crate::lang::lang()).unwrap_or(0);
        let zones = crate::when::Zone::ALL;
        let zone_now = zones.iter().position(|z| *z == crate::when::zone()).unwrap_or(0);
        let lang_cells: Vec<seg::Cell> = langs.iter().map(|l| seg::Cell::from(l.label())).collect();
        let zone_cells: Vec<seg::Cell> = zones.iter().map(|z| seg::Cell::from(zone_label(*z))).collect();
        let example = fill1(Key::ZoneExample, &crate::when::when(wall_secs()));
        let (mut pick_lang, mut pick_zone) = (None, None);
        let zone_say = if crate::when::zone() == crate::when::Zone::System && !crate::when::system_read() { t(Key::ZoneSystemUnread).to_string() } else { example };
        stagger(ui, 0, |ui| {
            card::form(ui, |ui, f| {
                f.row_with(ui, None, t(Key::Language), "", card::Value::None, |ui| pick_lang = seg::seg(ui, "set-lang", &lang_cells, cur));
                f.row_with(ui, None, t(Key::SetZoneTitle), &zone_say, card::Value::None, |ui| pick_zone = seg::seg(ui, "set-zone", &zone_cells, zone_now));
            });
        });
        if let Some(i) = pick_lang {
            if i != cur || self.shell.settings.lang != Some(langs[i]) {
                self.act(Action::SetLang { lang: langs[i] }, now);
            }
        }
        if let Some(i) = pick_zone {
            if i != zone_now || self.shell.settings.zone != Some(zones[i]) {
                self.act(Action::SetZone { zone: zones[i] }, now);
            }
        }
    }

    fn set_cli(&mut self, ui: &mut egui::Ui, now: f64) {
        self.set_cli_path(ui, now);
        self.set_cli_writes(ui, now);
        self.set_cli_anchor(ui, now);
    }

    fn set_cli_anchor(&mut self, ui: &mut egui::Ui, now: f64) {
        let all = crate::machine::CliAnchor::ALL;
        let cur = all.iter().position(|x| *x == self.shell.machine.cli_anchor).unwrap_or(0);
        let cells: Vec<seg::Cell> = all.iter().map(|x| seg::Cell::from(t(cli_anchor_label(*x)))).collect();
        let mut pick = None;
        stagger(ui, 2, |ui| {
            card::section(ui, t(Key::SetCliAnchoring), "", |ui| {
                card::form(ui, |ui, f| {
                    pick = f.with(ui, t(Key::SetCliAnchor), "", |ui| seg::seg(ui, "set-cli-anchor", &cells, cur));
                });
            });
        });
        if let Some(i) = pick.filter(|i| *i != cur) {
            self.act(Action::SetCliAnchor { to: all[i] }, now);
        }
    }

    fn set_cli_path(&mut self, ui: &mut egui::Ui, now: f64) {
        use zikaron_os::cli_path::State;
        if now - self.ux.cli_path_seen > 1.0 || self.shell.cli_path.is_none() {
            self.act(Action::ReadCliPath, now);
        }
        self.ux.cli_path_seen = now;
        let state = self.shell.cli_path.clone().unwrap_or(State::Off);
        let busy = self.shell.tasks.in_flight(crate::task::Kind::CliPath);
        let (on, can, say) = match &state {
            State::On => (true, true, t(Key::CliPathOnSay).to_string()),
            State::Off => (false, true, String::new()),
            State::Taken(what) => (false, true, fill1(Key::CliPathTakenSay, what)),
            State::Provided => (true, false, t(Key::CliPathProvidedSay).to_string()),
            State::Unsupported(_) => (false, false, t(Key::CliPathUnsupportedSay).to_string()),
        };
        let config = self.shell.ai_config.clone();
        let mut flip = false;
        let mut copy = false;
        stagger(ui, 0, |ui| {
            card::section(ui, t(Key::SetCliTerminal), "", |ui| {
                card::form(ui, |ui, f| {
                    flip = f.switch(ui, t(Key::SetCliPath), &say, on, can && !busy);
                    if let State::Unsupported(why) = &state {
                        f.fold(ui, "cli-path-why", t(Key::SetEvidence), "", |ui, f| f.note(ui, why));
                    }
                    copy = f.with(ui, t(Key::SetCliAiApp), "", |ui| {
                        let k = key::Key::new(t(Key::DoCopyAiConfig), Role::Secondary).enabled(config.is_some());
                        let r = key::show(ui, if config.is_some() { k } else { k.why(t(Key::CliPathUnsupportedSay)) });
                        zikaron_ui::probe::spot(ui.ctx(), "cli:copyAi", r.rect);
                        r.clicked()
                    });
                });
            });
        });
        if flip {
            self.act(Action::SetCliPath { on: !on }, now);
        }
        if let Some(c) = config.filter(|_| copy) {
            if self.shell.cli_alone_now() == crate::shell::CliAloneNow::Off {
                ui.ctx().copy_text(c);
                self.toasts.say(t(Key::SaidCopiedAiConfig), Tone::Note, now);
            } else {
                self.pass_open();
                self.ux.pass_for_ai = true;
                self.ux.pass_note = fill1(Key::CliPassAiNote, &crate::when::when(crate::lastread::now_secs()));
            }
        }
    }

    fn set_cli_writes(&mut self, ui: &mut egui::Ui, now: f64) {
        let path = self.shell.machine.cli_path();
        let alone = path == crate::machine::CliPath::Alone;
        let desktop = path == crate::machine::CliPath::Desktop;
        let unlocked = self.shell.unlocked();
        let alone_say = match (alone, self.shell.credential) {
            (true, Some(c)) => cli_expiry_said(c.expires),
            _ => String::new(),
        };
        let desktop_say = match (alone, desktop, self.shell.door_why()) {
            (true, _, _) => t(Key::SetCliDesktopGrey).to_string(),
            (false, true, Some(crate::door::DoorWhy::TooLong)) => t(Key::SetCliDoorLong).to_string(),
            (false, true, Some(crate::door::DoorWhy::Refused(w))) => fill1(Key::SetCliDoorShut, &w),
            _ => String::new(),
        };
        let (mut flip_desktop, mut flip_alone) = (false, false);
        stagger(ui, 1, |ui| {
            card::section(ui, t(Key::SetCliLevel), "", |ui| {
                card::form(ui, |ui, f| {
                    flip_desktop = f.switch(ui, t(Key::SetCliDesktop), &desktop_say, desktop, unlocked && !alone);
                    flip_alone = f.switch(ui, t(Key::SetCliAlone), &alone_say, alone, unlocked);
                });
            });
        });
        let now_state = self.shell.cli_alone_now();
        let rows = self.shell.passes.clone();
        let (mut add, mut drop): (bool, Option<String>) = (false, None);
        stagger(ui, 2, |ui| {
            card::section(ui, t(Key::CliPasses), "", |ui| {
                card::form(ui, |ui, f| {
                    if now_state == crate::shell::CliAloneNow::Off {
                        let _ = f.info(ui, t(Key::CliPassFirstOn), card::Show::Empty(""));
                    }
                    for r in &rows {
                        let (_, hit) = f.line(ui, None, &r.note, &fill1(Key::SaidRestAdded, &crate::when::when(r.added)), false, false, c(C::Ink), |ui| {
                            let k = key::key(ui, t(Key::V2DeleteGo), Role::Secondary, unlocked);
                            zikaron_ui::probe::spot(ui.ctx(), &format!("cli:drop:{}", r.tag), k.rect);
                            k.clicked()
                        });
                        if hit {
                            drop = Some(r.tag.clone());
                        }
                    }
                    let on = now_state != crate::shell::CliAloneNow::Off && unlocked;
                    let r = f.go(ui, t(Key::CliPassAddGo), "", on, t(Key::CliPassFirstOn));
                    zikaron_ui::probe::spot(ui.ctx(), "cli:add", r.rect);
                    add = r.clicked() && on;
                });
            });
        });
        if add {
            self.pass_open();
        }
        if let Some(tag) = drop {
            self.act(Action::CliPassDrop { tag }, now);
        }
        if flip_desktop {
            self.act(Action::SetCliDesktop { on: !desktop }, now);
        }
        if flip_alone {
            if alone {
                self.act(Action::CliAloneOff, now);
            } else {
                self.cli_open();
            }
        }
    }

    pub(super) fn go_cli_switches(&mut self, now: f64) {
        self.go(Place::Settings(Section::Cli), now);
    }

    pub(super) fn cli_open(&mut self) {
        self.sheets_clear();
        self.ux.cli_sheet = true;
        self.ux.cli_span = Default::default();
        self.ux.cli_never = false;
    }

    pub(super) fn pass_open(&mut self) {
        self.sheets_clear();
        self.ux.pass_open = true;
        self.ux.pass_for_ai = false;
        self.ux.pass_note.clear();
        self.ux.pass_pin.clear();
        self.ux.pass_out = None;
        self.ux.pass_trouble = None;
    }

    pub(super) fn pass_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if !self.ux.pass_open {
            self.ux.pass_out = None;
            return;
        }
        if self.ux.pass_for_ai {
            if let Some(pass) = self.ux.pass_out.take() {
                match self.shell.ai_config.as_deref().and_then(|c| crate::action::ai_config_with_pass(c, pass.expose())) {
                    Some(config) => {
                        ctx.copy_text(config);
                        self.toasts.say(t(Key::SaidCopiedAiConfigKeyed), Tone::Note, now);
                    }
                    None => self.toasts.say(t(Key::CliPathUnsupportedSay), Tone::Bad, now),
                }
                drop(pass);
                self.ux.pass_open = false;
                self.ux.pass_for_ai = false;
                self.ux.pass_pin.clear();
                return;
            }
        }
        let len = crate::keybox::PIN_LEN;
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Vault);
        let trouble = self.ux.pass_trouble.clone();
        let made = self.ux.pass_out.as_ref().map(|p| p.expose().to_string());
        let (mut go, mut close) = (false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("cli-pass", tk::SHEET_NARROW),
            self,
            |ui, me| {
                sheet::title(ui, t(if me.ux.pass_for_ai { Key::DoCopyAiConfig } else { Key::CliPassAddGo }), "");
                match &made {
                    Some(text) => {
                        let mut copied = None;
                        fields::stack(ui, |ui, st| {
                            st.field(ui, t(Key::CliPasses), None, None, |ui, w| copied = card::whole(ui, text, w));
                        });
                        tell_copied(ui.ctx(), copied);
                        sheet::foot_note(ui, t(Key::CliPassOnce));
                    }
                    None => {
                        fields::stack(ui, |ui, st| {
                            st.field(ui, t(Key::KitNoteLabel), None, None, |ui, w| fields::line(ui, &mut me.ux.pass_note, "", w, fields::Look::default()));
                            let _ = pin_field(st, ui, t(Key::IdPinGate), None, "cli-pass-pin", &mut me.ux.pass_pin, me.ux.pin_shake, !busy);
                        });
                    }
                }
                if let Some(f) = &trouble {
                    states::err_box(ui, "cli-pass-trouble", f.human(), &f.next(), t(Key::U3RawError), &f.shown());
                }
            },
            |ui, me| {
                if made.is_some() {
                    close = page::Page::new().primary(ui, t(Key::WizFinish)).1.clicked();
                    return;
                }
                let ready = !me.ux.pass_note.trim().is_empty() && me.ux.pass_pin.chars() == len && !busy;
                let why = still_needs(&[(me.ux.pass_note.trim().is_empty(), Key::KitNoteLabel), (me.ux.pass_pin.chars() != len, Key::IdPinGate)]);
                let r = page::Page::new().primary_why(ui, t(Key::CliPassAddGo), ready, &why).1;
                zikaron_ui::probe::spot(ui.ctx(), "cli:passGo", r.rect);
                go = r.clicked() && ready;
                close = key::key(ui, t(Key::CfBack), Role::Secondary, !busy).clicked();
            },
        );
        if close || (out.esc && !busy) {
            self.ux.pass_open = false;
            self.ux.pass_for_ai = false;
            self.ux.pass_out = None;
            self.ux.pass_pin.clear();
            return;
        }
        if go {
            self.ux.pass_trouble = None;
            let pin = std::mem::take(&mut self.ux.pass_pin);
            let r = self.act(Action::CliPassAdd { note: self.ux.pass_note.clone(), pin }, now);
            self.vault_or(VaultSite::CliPass, r, now);
        }
    }

    pub(super) fn cli_close(&mut self) {
        self.ux.cli_sheet = false;
        self.ux.cli_pin.clear();
        self.ux.cli_trouble = None;
    }

    pub(super) fn cli_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if !self.ux.cli_sheet {
            return;
        }
        let len = crate::keybox::PIN_LEN;
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Vault);
        let span = cli_span(&self.ux.cli_span, self.ux.cli_never);
        let lasts = match span {
            Span::Lasts(l) => Some(l),
            _ => None,
        };
        let pin_full = self.ux.cli_pin.chars() == len;
        let trouble = self.ux.cli_trouble.clone();
        let said = cli_span_said(span, (self.shell.clock)());
        let (mut go, mut close) = (false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("cli-alone", tk::SHEET_NARROW).keys_above(tk::S5),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::SetCliAlone), "");
                ui.add_space(tk::S4 - 6.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let never = me.ux.cli_never;
                    let units: Vec<&str> = CLI_SPAN.iter().map(|(unit, _, _)| t(*unit)).collect();
                    let bad: Vec<bool> = (0..CLI_SPAN.len()).map(|i| matches!(span, Span::Over(o) if o[i])).collect();
                    let line = (!said.0.is_empty()).then(|| (said.0.as_str(), said.1));
                    fields::stack(ui, |ui, st| st.field(ui, t(Key::U3Window), None, line, |ui, _| zikaron_ui::lasts::boxed(ui, "cli-span", &mut me.ux.cli_span, &units, "0", &bad, !never)));
                    ui.add_space(tk::S3);
                    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), Type::Note.line()), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let w = ui.add(egui::Label::new(egui::RichText::new(t(Key::CliLastsNever)).font(Type::Note.font()).color(c(C::Ink2))).selectable(false).sense(egui::Sense::click()));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let s = toggle::switch(ui, me.ux.cli_never, true);
                            if s.clicked() || w.clicked() {
                                me.ux.cli_never = !me.ux.cli_never;
                            }
                        });
                    });
                    ui.add_space(tk::S5);
                    fields::name(ui, t(Key::IdPinGate), None);
                    ui.add_space(tk::S3);
                    let _ = zikaron_ui::sheetparts::pins(ui, "cli-alone-pin", &mut me.ux.cli_pin, len, me.ux.pin_shake, !busy);
                });
                if let Some(f) = &trouble {
                    states::err_box(ui, "cli-trouble", f.human(), &f.next(), t(Key::U3RawError), &f.shown());
                }
            },
            |ui, _me| {
                let why = still_needs(&[(lasts.is_none(), Key::U3Window), (!pin_full, Key::IdPinGate)]);
                go = page::Page::new().primary_why(ui, t(Key::DoCliAloneOn), pin_full && lasts.is_some() && !busy, &why).1.clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                if busy {
                    sheet::foot_note(ui, t(Key::VaultBusy));
                }
            },
        );
        if close || out.esc {
            self.cli_close();
        } else if let (true, Some(lasts)) = (go, lasts) {
            let pin = std::mem::take(&mut self.ux.cli_pin);
            let r = self.act(Action::SetCliAlone { lasts, pin }, now);
            self.vault_or(VaultSite::Cli, r, now);
        }
    }

    fn set_appearance(&mut self, ui: &mut egui::Ui, now: f64) {
        let all = [skin::Appearance::Light, skin::Appearance::Dark, skin::Appearance::System];
        let cur = all.iter().position(|a| *a == self.appearance()).unwrap_or(0);
        let cells: Vec<seg::Cell> = all.iter().map(|a| seg::Cell::from(appearance_label(a.as_str()))).collect();
        let mut pick = None;
        stagger(ui, 0, |ui| {
            card::form(ui, |ui, f| {
                f.row_with(ui, None, t(Key::SetAppearance), "", card::Value::None, |ui| pick = seg::seg(ui, "set-appearance", &cells, cur));
            });
        });
        if let Some(i) = pick {
            if i != cur {
                self.act(Action::SetAppearance { appearance: all[i].as_str().to_string() }, now);
            }
        }
    }

    pub(super) fn read_net_new_open(&mut self) {
        self.ux.readnet = ReadNetUx { sheet: Some((None, Draft::default())), armed: false };
    }

    pub(super) fn read_net_registry_typed(&mut self, text: &str) {
        if let Some((_, d)) = self.ux.readnet.sheet.as_mut() {
            d.registry = text.to_string();
        }
    }

    pub(super) fn net_sheet_open(&self) -> bool {
        self.ux.net_sheet || self.ux.readnet.sheet.is_some() || self.ux.publish_sheet || self.ux.publish_edit
    }

    pub(super) fn net_sheets_clear(&mut self) {
        self.ux.net_sheet = false;
        self.ux.basis_waiting = false;
        self.ux.readnet = ReadNetUx::default();
        self.ux.publish_sheet = false;
        self.ux.publish_edit = false;
    }

    pub(super) fn net_edit_open(&mut self) {
        let s = &self.shell.settings;
        self.ux.basis_chain = s.chain_id.map(|n| n.to_string()).unwrap_or_default();
        self.ux.basis_registry = s.registry.map(|a| a.hex()).unwrap_or_default();
        self.ux.basis_from = if s.chain_id.is_some() || s.registry.is_some() { s.from_block.to_string() } else { String::new() };
        self.typed.endpoints = s.endpoints.join(" ");
        self.ux.basis_custom = false;
        self.ux.basis_waiting = false;
        self.ux.net_sheet = true;
    }

    fn net_fill(&mut self, d: &crate::deploy::Deployment) {
        let c = d.cells();
        self.ux.basis_chain = c.chain;
        self.ux.basis_registry = c.registry;
        self.ux.basis_from = c.from;
        self.typed.endpoints = c.endpoints;
    }

    fn net_missing(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.ux.basis_chain.trim().parse::<u64>().is_err() {
            v.push(t(Key::BasisChain));
        }
        if self.ux.basis_from.trim().parse::<u64>().is_err() {
            v.push(t(Key::BasisFrom));
        }
        if crate::key::Address::parse(&self.ux.basis_registry).is_none() {
            v.push(t(Key::SetRegistry));
        }
        v
    }

    fn net_row_typed(&self) -> Option<usize> {
        crate::deploy::KNOWN.iter().position(|d| {
            let c = d.cells();
            c.chain == self.ux.basis_chain.trim() && c.registry.eq_ignore_ascii_case(self.ux.basis_registry.trim()) && c.from == self.ux.basis_from.trim() && c.endpoints.split_whitespace().eq(self.typed.endpoints.split_whitespace())
        })
    }

    pub(super) fn net_edit_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if !self.ux.net_sheet {
            return;
        }
        let checking = self.shell.tasks.in_flight(crate::task::Kind::Basis);
        let typed = (self.ux.basis_chain.trim().parse::<u64>().ok(), crate::key::Address::parse(&self.ux.basis_registry));
        if self.ux.basis_waiting && !checking {
            self.ux.basis_waiting = false;
            if let Some((c, r, Some(crate::widex::Reading::Agreed(_) | crate::widex::Reading::Single))) = self.shell.basis_read {
                if typed == (Some(c), Some(r)) {
                    self.ux.net_sheet = false;
                    return;
                }
            }
        }
        let reading = self.shell.basis_read.filter(|(c, r, _)| typed == (Some(*c), Some(*r))).map(|(_, _, x)| match x {
            Some(r @ (crate::widex::Reading::Agreed(_) | crate::widex::Reading::Single)) => (Mark::Ok, r.key()),
            Some(crate::widex::Reading::Fingerprint) => (Mark::Bad, Key::BadgeFingerprint),
            Some(crate::widex::Reading::Down) => (Mark::Warn, Key::BadgeDown),
            None => (Mark::Warn, Key::BadgeUnchecked),
        });
        let phase = self.phase_of(crate::task::Kind::Basis);
        let (mut save, mut close) = (false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("network-edit", 500.0),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::NetEditTitle), "");
                if let Some((_, k)) = reading.filter(|(m, _)| *m == Mark::Warn) {
                    states::banner(ui, states::Banner::Warn, &format!("{} \u{b7} {}", t(Key::SetRegistry), t(k)), |_| ());
                }
                let row = me.net_row_typed();
                let custom = me.ux.basis_custom || row != Some(0);
                let cells = [seg::Cell::from(t(crate::deploy::KNOWN[0].label)), seg::Cell::from(t(Key::U3Custom))];
                let (mut pick, mut preset) = (None, None);
                fields::stack(ui, |ui, st| {
                    ui.horizontal(|ui| {
                        pick = seg::seg(ui, "network-kind", &cells, usize::from(custom));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let b = if custom { egui::UiBuilder::new() } else { egui::UiBuilder::new().invisible() };
                            ui.scope_builder(b, |ui| preset = preset_menu(ui, "network-preset", row));
                        });
                    });
                    let mono = fields::Look { mono: true, ..Default::default() };
                    let reg_bad = crate::addressedx::not_address(&me.ux.basis_registry, crate::addressedx::Takes::Address);
                    let check = if reg_bad { Some((t(Key::KitToWhyAddress), true)) } else { reading.filter(|(m, _)| *m != Mark::Warn).map(|(m, k)| (t(k), m == Mark::Bad)) };
                    st.pair(
                        ui,
                        |ui, st| st.field(ui, t(Key::BasisChain), None, None, |ui, w| fields::line(ui, &mut me.ux.basis_chain, "", w, mono)),
                        |ui, st| st.field(ui, t(Key::BasisFrom), None, None, |ui, w| fields::line(ui, &mut me.ux.basis_from, "", w, mono)),
                    );
                    st.field(ui, t(Key::SetRegistry), None, check, |ui, w| fields::line(ui, &mut me.ux.basis_registry, "", w, fields::Look { bad: reg_bad, ..mono }));
                });
                match pick {
                    Some(0) => {
                        me.ux.basis_custom = false;
                        me.net_fill(&crate::deploy::KNOWN[0]);
                    }
                    Some(_) if !custom => {
                        me.ux.basis_custom = true;
                        me.ux.basis_chain.clear();
                        me.ux.basis_registry.clear();
                        me.ux.basis_from.clear();
                        me.typed.endpoints = me.shell.settings.endpoints.join(" ");
                    }
                    _ => {}
                }
                if let Some(i) = preset {
                    me.net_fill(&crate::deploy::KNOWN[i]);
                }
            },
            |ui, me| {
                let missing = me.net_missing();
                let k = key::Key::new(t(Key::ReadNetSave), Role::Primary).phase(phase).enabled(missing.is_empty());
                let still = registry_still(&me.ux.basis_registry, &missing);
                save = key::show(ui, if missing.is_empty() { k } else { k.why(&still) }).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
            },
        );
        if close || out.esc {
            self.ux.net_sheet = false;
            return;
        }
        if !save || checking {
            return;
        }
        let s = self.shell.settings.clone();
        let basis_same = typed.0 == s.chain_id && typed.1 == s.registry && self.ux.basis_from.trim().parse::<u64>().ok() == Some(s.from_block);
        let nodes_same = self.typed.endpoints.split_whitespace().eq(s.endpoints.iter().map(String::as_str));
        let mut kept = true;
        if !basis_same {
            let a = Action::SetBasis { chain: self.ux.basis_chain.clone(), registry: self.ux.basis_registry.clone(), from_block: self.ux.basis_from.clone() };
            match self.act(a, now) {
                Applied::Basis { .. } => {}
                Applied::Started(crate::task::Kind::Basis) => {
                    self.ux.basis_waiting = true;
                    kept = false;
                }
                _ => return,
            }
        }
        if !nodes_same {
            let a = Action::SetEndpoints { specs: self.typed.endpoints.clone() };
            if !matches!(self.act(a, now), Applied::Endpoints(_)) {
                return;
            }
            if self.shell.tasks.in_flight(crate::task::Kind::Basis) {
                self.ux.basis_waiting = true;
                kept = false;
            }
        }
        if kept {
            self.ux.net_sheet = false;
        }
    }

    pub(super) fn set_chain_group(&mut self, ui: &mut egui::Ui, now: f64) {
        let s = self.shell.settings.clone();
        let bare = s.chain_id.is_none() && s.registry.is_none() && s.endpoints.is_empty();
        let registry = s.registry.map(|a| a.hex());
        let (mut edit, mut copied, mut flip, mut move_chain) = (false, None, false, false);
        stagger(ui, 4, |ui| {
            card::section(ui, t(Key::NetGroup), "", |ui| {
                card::form(ui, |ui, f| {
                    let k = key::Key::new(t(Key::DoEdit), if bare { Role::Primary } else { Role::Secondary });
                    let chain = s.chain_id.map(|c| c.to_string()).unwrap_or_else(|| t(Key::SetNone).to_string());
                    edit = f.act_value(ui, t(Key::BasisChain), "", &chain, k).clicked();
                    copied = match registry.as_deref() {
                        Some(r) => f.info(ui, t(Key::SetRegistry), card::Show::Short(r)),
                        None => f.info(ui, t(Key::SetRegistry), card::Show::Empty(t(Key::SetNone))),
                    };
                    flip = f.switch(ui, t(Key::SetAutoAnchor), "", s.auto_anchor, true);
                    if !s.earlier.is_empty() {
                        let said = s.earlier.iter().map(|e| format!("{} · {}", fill1(Key::ChainNumbered, &e.chain_id.to_string()), zikaron::hexfmt::encode(&e.registry))).collect::<Vec<_>>().join("\n");
                        let _ = f.info(ui, t(Key::ChainEarlier), card::Show::Text(&said));
                    }
                    move_chain = f.go(ui, t(Key::ChainMoveTitle), "", !bare, "").clicked();
                });
            });
        });
        tell_copied(ui.ctx(), copied);
        if flip {
            self.act(Action::SetAutoAnchor { on: !s.auto_anchor }, now);
        }
        if edit {
            self.net_edit_open();
        }
        if move_chain {
            self.sheets_clear();
            self.ux.move_open = true;
        }
    }

    pub(super) fn set_read_nets(&mut self, ui: &mut egui::Ui) {
        let nets = self.shell.read_nets.clone().unwrap_or_default();
        let reads = self.shell.net_reads.clone();
        let (mut open, mut add) = (None, false);
        card::section(ui, t(Key::ReadNetTitle), "", |ui| {
            card::form(ui, |ui, f| {
                for n in &nets {
                    let k = (n.chain_id, n.registry);
                    let badge = reads.iter().find(|(c, r, _)| *c == k.0 && *r == k.1).map(|(_, _, x)| (t(x.key()), read_tone(*x)));
                    let chain = fill1(Key::ChainNumbered, &n.chain_id.to_string());
                    let (row, _) = f.line(ui, None, &n.name(), &chain, false, true, c(C::Ink), |ui| {
                        if let Some((s, tone)) = badge {
                            mark::pill(ui, s, tone);
                        }
                    });
                    if row.clicked() {
                        open = Some(n.clone());
                    }
                }
                add = f.add_row(ui, t(Key::ReadNetAdd)).clicked();
            });
        });
        if let Some(n) = open {
            self.ux.readnet = ReadNetUx { sheet: Some((Some((n.chain_id, n.registry)), Draft::of(&n))), armed: false };
        } else if add {
            self.ux.readnet = ReadNetUx { sheet: Some((None, Draft::default())), armed: false };
        }
    }

    pub(super) fn read_net_sheet(&mut self, ctx: &egui::Context, now: f64) {
        let Some((was, _)) = self.ux.readnet.sheet.clone() else { return };
        let armed = self.ux.readnet.armed;
        let reading = was.and_then(|k| self.shell.net_reads.iter().find(|(c, r, _)| *c == k.0 && *r == k.1).map(|(_, _, x)| *x));
        let phase = self.phase_of(crate::task::Kind::ReadNet);
        let saved = was.and_then(|k| self.shell.read_nets.as_ref().and_then(|v| v.iter().find(|n| n.is(k.0, &k.1)).cloned()));
        let title = saved.map(|n| n.name()).unwrap_or_else(|| t(Key::ReadNetNew).to_string());
        let (mut read, mut act, mut close, mut del) = (None, None, false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("read-network", 500.0),
            self,
            |ui, me| {
                let Some((_, d)) = me.ux.readnet.sheet.as_mut() else { return };
                sheet::title(ui, &title, "");
                if let Some(k) = was {
                    card::form(ui, |ui, f| {
                        let key = key::Key::new(t(Key::StageReadChain), Role::Secondary).phase(phase);
                        let said = reading.map(|x| t(x.key())).unwrap_or(t(Key::SetNotRead));
                        if f.act(ui, t(Key::SetChainRead), said, key).clicked() {
                            read = Some(Action::ReadReadNetwork { chain: k.0, registry: k.1.hex() });
                        }
                    });
                }
                net_cells(ui, d);
            },
            |ui, me| {
                let Some((_, d)) = me.ux.readnet.sheet.as_ref() else { return };
                let missing = d.missing();
                let k = key::Key::new(t(Key::ReadNetSave), Role::Primary).enabled(missing.is_empty());
                let still = registry_still(&d.registry, &missing);
                if key::show(ui, if missing.is_empty() { k } else { k.why(&still) }).clicked() {
                    act = Some(d.save(was));
                }
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                if was.is_some() {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        del = if armed {
                            paint::text(ui, t(Key::ReadNetRemoveAsk), Type::Body, c(C::Ink));
                            key::key(ui, t(Key::KitRemove), Role::Danger, true).clicked()
                        } else {
                            key::key(ui, t(Key::KitRemove), Role::Secondary, true).clicked()
                        };
                    });
                }
            },
        );
        if close || out.esc {
            self.ux.readnet = ReadNetUx::default();
            return;
        }
        if del {
            if !armed {
                self.ux.readnet.armed = true;
                return;
            }
            act = was.map(|k| Action::RemoveReadNetwork { chain: k.0, registry: k.1.hex() });
        }
        let Some(a) = act.or(read) else { return };
        let ends = matches!(a, Action::SaveReadNetwork { .. } | Action::RemoveReadNetwork { .. });
        if let Applied::ReadNets(_) = self.act(a, now) {
            if ends {
                self.ux.readnet = ReadNetUx::default();
            }
        }
    }

    pub(super) fn add_stated_network(&mut self, at: &crate::kitsindex::AnchoredOn, now: f64) {
        let d = Draft { preset: None, name: String::new(), chain: at.chain_id.to_string(), registry: at.registry.clone(), from: at.from_block.to_string(), nodes: String::new() };
        self.go(Place::Settings(Section::Network), now);
        self.ux.readnet = ReadNetUx { sheet: Some((None, d)), armed: false };
    }

    pub(super) fn set_publish(&mut self, ui: &mut egui::Ui) {
        let saved = self.shell.settings.publish.clone();
        let last = self.shell.published.as_ref().map(|(_, r)| if r.complete() { t(Key::PublishOk) } else { t(Key::PublishPartial) });
        let (mut edit, mut check) = (false, false);
        card::form(ui, |ui, f| {
            edit = match saved.as_deref() {
                Some(u) => f.act_path(ui, t(Key::SetPublish), u, key::Key::new(t(Key::DoEdit), Role::Secondary)),
                None => f.act(ui, t(Key::SetPublish), t(Key::SetNone), key::Key::new(t(Key::DoEdit), Role::Secondary)),
            }
            .clicked();
            check = f.go(ui, t(Key::DoCheckPublished), last.unwrap_or(""), saved.is_some(), t(Key::SetPublish)).clicked();
        });
        if edit {
            self.typed.publish = saved.clone().unwrap_or_default();
            self.ux.publish_edit = true;
        }
        if check {
            if self.typed.publish_local.trim().is_empty() {
                if let Some(p) = self.shell.home.as_ref().and_then(|h| crate::kitx::latest_kit(&h.dir(crate::home::Slot::Kits))) {
                    self.typed.publish_local = p.display().to_string();
                }
            }
            self.ux.publish_sheet = true;
        }
    }

    pub(super) fn publish_edit_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if !self.ux.publish_edit {
            return;
        }
        let typed = self.typed.publish.trim().to_string();
        let shape = (!typed.is_empty()).then(|| crate::fetchx::base_of(&typed).err()).flatten();
        let said = shape.as_ref().map(|f| f.human().to_string());
        let differs = typed != self.shell.settings.publish.clone().unwrap_or_default();
        let (mut save, mut close) = (false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("publish-edit", tk::SHEET_W),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::SetPublish), "");
                fields::stack(ui, |ui, st| {
                    st.field(ui, t(Key::SetPublish), None, said.as_deref().map(|s| (s, true)), |ui, w| {
                        let look = fields::Look { mono: true, bad: said.is_some(), ..Default::default() };
                        fields::line(ui, &mut me.typed.publish, "https://\u{2026}", w, look)
                    });
                });
            },
            |ui, _me| {
                let why = match (&said, differs) {
                    (Some(s), _) => s.clone(),
                    (None, false) => t(Key::PublishSame).to_string(),
                    _ => String::new(),
                };
                save = key::show(ui, key::Key::new(t(Key::DoSetPublish), Role::Primary).enabled(differs && shape.is_none()).why(&why)).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
            },
        );
        if close || out.esc {
            self.ux.publish_edit = false;
        } else if save {
            self.act(Action::SetPublish { url: typed }, now);
            self.ux.publish_edit = false;
        }
    }

    pub(super) fn publish_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if !self.ux.publish_sheet {
            return;
        }
        let k = crate::task::Kind::Publish;
        let running = self.shell.tasks.in_flight(k);
        let phase = self.phase_of(k);
        let (mut go, mut close, mut picked) = (false, false, None);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("publish-check", tk::SHEET_W),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::DoCheckPublished), "");
                let local = me.typed.publish_local.clone();
                card::form(ui, |ui, f| f.folder(ui, t(Key::SetPublishLocal), &local, t(Key::PickNone)));
                me.stage_line(ui, k);
                if running {
                    return;
                }
                if let Some(f) = me.shell.failed.get(&k) {
                    states::err_box(ui, "publish-err", t(Key::PublishUnreachable), f.human(), t(Key::U3RawError), &f.shown());
                } else if let Some((_, r)) = me.shell.published.as_ref() {
                    if r.complete() {
                        states::okline(ui, Mark::Ok, &format!("{} \u{b7} {}", t(Key::PublishOk), fill1(Key::PublishOkSay, &r.total.to_string())));
                    } else {
                        states::okline(ui, Mark::Bad, &format!("{} \u{b7} {}", t(Key::PublishPartial), fill2(Key::PublishPartialSay, &r.missing.len().to_string(), &r.differ.len().to_string())));
                        if !r.missing.is_empty() {
                            hint(ui, &fill1(Key::PublishMissingList, &r.missing.join(t(Key::ListJoin))));
                        }
                        if !r.differ.is_empty() {
                            hint(ui, &fill1(Key::PublishDifferList, &r.differ.join(t(Key::ListJoin))));
                        }
                    }
                }
            },
            |ui, me| {
                let why = still_needs(&[(me.typed.publish_local.trim().is_empty(), Key::Folder)]);
                go = key::show(ui, key::Key::new(t(Key::DoCheckShort), Role::Primary).enabled(!me.typed.publish_local.trim().is_empty()).phase(phase).why(&why)).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                picked = foot_pick(ui, "publish-local", crate::platform::Pick::Folder);
            },
        );
        if let Some(p) = picked {
            self.typed.publish_local = p;
        }
        if close || out.esc {
            self.ux.publish_sheet = false;
        } else if go && !running {
            let a = Action::CheckPublished { local: self.typed.publish_local.clone() };
            self.act(a, now);
        }
    }

    fn setting_rows_follow(&mut self) {
        let owner = (self.shell.home.as_ref().map(|h| h.root().to_path_buf()), self.shell.identities.as_ref().and_then(|r| r.now().map(|(x, _)| x.id.clone())), Some(self.shell.settings.role));
        if self.ux.edits_owner.as_ref() != Some(&owner) {
            self.ux.audit_edit = fields::SavedEdit::default();
            self.ux.review_edit = fields::SavedEdit::default();
            self.ux.cap_edit = fields::SavedEdit::default();
            self.ux.edits_owner = Some(owner);
        }
    }

    fn set_notify(&mut self, ui: &mut egui::Ui, now: f64) {
        self.setting_rows_follow();
        let s = self.shell.settings.clone();
        let every = |n: u64| if n == 0 { String::new() } else { n.to_string() };
        let check = |x: &str| if x.is_empty() || x.parse::<u64>().is_ok() { Ok(()) } else { Err(t(Key::U3EveryMustBeNumber)) };
        let key_w = [Key::DoSetAuditEvery, Key::DoSetReviewEvery].iter().map(|k| key::size_of(ui, &key::Key::new(t(*k), Role::Secondary)).x).fold(92.0_f32, f32::max);
        let words = |save: Key| card::SettingWords { save: t(save), saved: t(Key::SettingSaved), unit: t(Key::UnitSeconds), hint: t(Key::SetEveryOff), key_w };
        let (mut save_audit, mut save_review) = (None, None);
        let ux = &mut self.ux;
        stagger(ui, 0, |ui| {
            card::form(ui, |ui, f| {
                save_audit = f.setting(ui, t(Key::PageAudit), &mut ux.audit_edit, &every(s.audit_every), 110.0, words(Key::DoSetAuditEvery), check);
                save_review = f.setting(ui, t(Key::SetReview), &mut ux.review_edit, &every(s.review_every), 110.0, words(Key::DoSetReviewEvery), check);
            });
        });
        let read = |x: &str| if x.is_empty() { Some(0) } else { x.parse::<u64>().ok() };
        if let Some(n) = save_audit.as_deref().and_then(read) {
            if !matches!(self.act(Action::SetAuditEvery { secs: n }, now), Applied::Trouble(_)) {
                self.ux.audit_edit.land();
            }
        }
        if let Some(n) = save_review.as_deref().and_then(read) {
            if !matches!(self.act(Action::SetReviewEvery { secs: n.to_string() }, now), Applied::Trouble(_)) {
                self.ux.review_edit.land();
            }
        }
    }

    fn set_fetch(&mut self, ui: &mut egui::Ui, now: f64) {
        let Some(s) = self.shell.unfetched.as_ref() else { return };
        let title = super::watch::mark_title(s);
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Fetch);
        let (mut pick, mut go) = (false, false);
        card::card(ui, |ui| {
            ui.spacing_mut().item_spacing.y = tk::S3;
            states::banner(ui, states::Banner::Bad, &title, |_| ());
            hint(ui, t(Key::FetchLedgerHint));
            let (_, p) = width::line_then(ui, &mut self.typed.fetch_from, t(Key::BackupDrop), true, |ui| key::key(ui, t(Key::PickFile), Role::Secondary, true).clicked());
            pick = p;
            field(ui, t(Key::BackupPasswordPlain), None, |ui| input::secret_line(ui, &mut self.ux.fetch_pw, ""));
            keys_row(ui, |ui| go = key::key(ui, t(Key::DoFetchLedger), Role::Secondary, !busy && !self.typed.fetch_from.trim().is_empty() && !self.ux.fetch_pw.is_empty()).clicked());
            self.stage_line(ui, crate::task::Kind::Fetch);
        });
        if let Some(p) = path_answer(ui.ctx(), egui::Id::new("zikaron-path-fetch-from"), pick, crate::platform::Pick::File) {
            self.typed.fetch_from = p;
        }
        if go {
            self.ux.fetch_held = self.ux.fetch_pw.clone();
            let a = Action::FetchLedger { from: self.typed.fetch_from.clone(), password: std::mem::take(&mut self.ux.fetch_pw) };
            self.act(a, now);
        }
    }

    fn set_old_data(&mut self, ui: &mut egui::Ui, now: f64) {
        if self.shell.aside.is_empty() {
            return;
        }
        let rows: Vec<(String, String, std::path::PathBuf)> = self.shell.aside.iter().map(|a| (crate::when::day(a.at), fill2(Key::OldDataRow, &a.entries.to_string(), &a.queued.to_string()), a.path.clone())).collect();
        let mut open: Option<std::path::PathBuf> = None;
        card::section(ui, t(Key::OldDataTitle), "", |ui| {
            card::form(ui, |ui, f| {
                for (day, say, path) in &rows {
                    if f.row(ui, day, say, card::Value::NavWith(t(Key::NavLook))).clicked() {
                        open = Some(path.clone());
                    }
                }
            });
        });
        if let Some(p) = open {
            self.act(Action::ViewOldData { root: p.display().to_string() }, now);
        }
    }

    fn set_data(&mut self, ui: &mut egui::Ui, now: f64) {
        self.set_fetch(ui, now);
        self.set_old_data(ui, now);
        self.measure_if_due(now);
        let archive = self.shell.archive_now().cloned();
        let content = self.data_content();
        let strays = archive.as_ref().filter(|a| a.skipped > 0).map(|a| fill2(Key::SetStraysSay, &a.items.to_string(), &a.skipped.to_string()));
        let usage = archive.as_ref().map(|a| fill2(Key::SetUsageHuman, &size_say(a.bytes), &size_say(self.shell.settings.cap_bytes))).unwrap_or_else(|| t(Key::SetNotRead).to_string());
        let over = archive.as_ref().is_some_and(|a| a.bytes > self.shell.settings.cap_bytes);
        let seat_ok = !self.shell.seat_unseated();
        let held = self.shell.unfetched.is_some() || !self.shell.pen_here();
        let pen = if self.shell.unfetched.is_some() {
            t(Key::SetPenHeld)
        } else if !self.shell.pen_here() {
            t(Key::SetPenReadOnly)
        } else {
            t(Key::SetPenOpen)
        };
        let head = format!("{} \u{b7} {pen}", t(Key::DataProtectOn));
        let entries = archive.as_ref().map(|a| a.items.to_string()).unwrap_or_else(|| t(Key::SetNotRead).to_string());
        let reconciled = self.shell.reconciled.as_ref().map(|(label, _, n)| format!("{} \u{b7} {}", label_human(label), fill1(Key::U3EntriesCount, &n.to_string())));
        let last = self.shell.machine.backup.clone();
        let behind = self.backup_behind();
        let backup_say = match (&last, behind) {
            (Some(b), Some(n)) if n > 0 => format!("{} \u{b7} {}", crate::when::when(b.at), fill1(Key::U3EntriesCount, &n.to_string())),
            (Some(b), _) => crate::when::when(b.at),
            (None, _) => t(Key::DataBackupNever).to_string(),
        };
        let backup_fact = if last.is_some() && !behind.is_some_and(|n| n > 0) { card::Fact::Text(&backup_say) } else { card::Fact::Warn(&backup_say) };
        let reconciled_fact = reconciled.as_deref().map(card::Fact::Text).unwrap_or(card::Fact::Quiet(t(Key::NotRun)));
        let entries_fact = if archive.is_some() { card::Fact::Text(&entries) } else { card::Fact::Quiet(&entries) };
        let reconcile_phase = self.phase_of(crate::task::Kind::Reconcile);
        let mut reconcile = false;
        stagger(ui, 0, |ui| {
            card::form(ui, |ui, f| {
                let tint = if held { card::Tint::Warn } else { card::Tint::Ok };
                reconcile = f.summary(ui, Glyph::Lock, tint, &head, None, "", |ui| key::show(ui, key::Key::new(t(Key::DoReconcile), Role::Secondary).phase(reconcile_phase)).clicked());
                f.facts(ui, &[(t(Key::DataEntries), entries_fact), (t(Key::LastAudit), reconciled_fact), (t(Key::DataBackupLast), backup_fact)]);
            });
            self.stage_line(ui, crate::task::Kind::Reconcile);
        });
        if !seat_ok {
            zikaron_ui::callout::callout(ui, t(Key::IdSeatEmptyNote));
        }
        let author = self.shell.settings.role == crate::roles::Role::Author;
        self.backup_group(ui, now);
        let root = self.shell.home.as_ref().map(|h| h.root().display().to_string());
        let measure_phase = self.phase_of(crate::task::Kind::Archive);
        let (mut open, mut measure, mut flip, mut clean) = (None, false, false, false);
        let hide = self.shell.settings.hide_local_deletions;
        stagger(ui, 2, |ui| {
            card::form(ui, |ui, f| {
                let path = root.clone().unwrap_or_else(|| t(Key::HomeNotOpen).to_string());
                if f.act_path(ui, t(Key::SetHomeOfSeat), &path, key::Key::new(t(Key::SetChangeHome), Role::Secondary).enabled(seat_ok)).clicked() {
                    open = Some(DirSheet::Home);
                }
                let sub = strays.as_deref().unwrap_or("");
                measure = f.act_value(ui, t(Key::SetUsage), sub, &usage, key::Key::new(t(Key::DoMeasure), Role::Secondary).enabled(seat_ok).phase(measure_phase)).clicked();
                if author {
                    flip = f.switch(ui, t(Key::SetHideLocalDeletions), "", hide, seat_ok);
                }
                clean = f.go(ui, t(Key::CleanTitle), "", self.shell.unlocked(), "").clicked();
            });
            self.stage_line(ui, crate::task::Kind::Archive);
            if over {
                states::okline(ui, Mark::Bad, &usage);
            }
        });
        if clean {
            self.shell.cleaned = None;
            self.ux.landed.remove(&crate::task::Kind::Clean);
            self.ux.clean_sheet = true;
            self.act(Action::CleanExports { yes: String::new() }, now);
        }
        let grantee = !author;
        self.setting_rows_follow();
        let cap = self.shell.settings.cap_bytes;
        let migrating = self.phase_of(crate::task::Kind::Migrate);
        let (mut set_cap, mut recheck) = (None, false);
        let cap_edit = &mut self.ux.cap_edit;
        stagger(ui, 3, |ui| {
            card::section(ui, t(Key::U3MoreOptions), "", |ui| {
                card::form(ui, |ui, f| {
                    let words = card::SettingWords { save: t(Key::DoSetCap), saved: t(Key::SettingSaved), unit: t(Key::UnitBytes), hint: "", key_w: 92.0 };
                    let check = |x: &str| if !seat_ok { Err("") } else if x.parse::<u64>().is_ok() { Ok(()) } else { Err(t(Key::CapMustBeNumber)) };
                    set_cap = f.setting(ui, t(Key::SetCap), cap_edit, &cap.to_string(), 130.0, words, check);
                    if f.act(ui, t(Key::MigrateLabel), "", key::Key::new(t(Key::DoMigrate), Role::Secondary).enabled(seat_ok).phase(migrating).busy_text(t(Key::SetMigrating))).clicked() {
                        open = Some(DirSheet::Migrate);
                    }
                    if f.act(ui, t(Key::AdoptGroup), "", key::Key::new(t(Key::DoAdopt), Role::Secondary).enabled(seat_ok)).clicked() {
                        open = Some(DirSheet::Adopt);
                    }
                    if grantee && f.go(ui, t(Key::V2ImportGrantDir), "", seat_ok, "").clicked() {
                        open = Some(DirSheet::GrantDir);
                    }
                    recheck = f.act(ui, t(Key::RecheckRow), "", key::Key::new(t(Key::DoRecheckAll), Role::Secondary)).clicked();
                });
            });
        });
        let lock = match self.shell.lock.as_ref() {
            None => t(Key::HomeNoLock).to_string(),
            Some(l) => format!("{} \u{b7} {}", l.mode().as_str(), l.holder()),
        };
        let miss = self.shell.home.as_ref().map(|h| h.missing()).unwrap_or_default();
        let none = || Val::text(t(Key::SetNone));
        stagger(ui, 4, |ui| {
            details_card(
                ui,
                "data-details",
                &[
                    (t(Key::SetHomeOfSeat), root.clone().map(Val::mono).unwrap_or_else(|| Val::text(t(Key::HomeNotOpen)))),
                    (t(Key::DataContent), Val::text(content.clone())),
                    (t(Key::DataEncrypt), Val::figure(t(Key::DataEncryptHow).to_string())),
                    (t(Key::DataBackupFile), last.as_ref().map(|b| Val::mono(b.path.clone())).unwrap_or_else(none)),
                    (t(Key::DataBackupHow), Val::figure(t(Key::DataBackupHowSay).to_string())),
                    (t(Key::HomeWhere), Val::text(t(Key::HomeWhereNote))),
                    (t(Key::HomeInstance), Val::mono(lock.clone())),
                    (t(Key::HomeMissingRooms), if miss.is_empty() { Val::text(t(Key::Nothing)) } else { Val::mono(miss.join(" ")) }),
                ],
            )
        });
        if let Some(d) = open {
            self.dir_open(d);
        }
        if flip {
            self.act(Action::SetHideLocalDeletions { on: !hide }, now);
        }
        if measure {
            self.act(Action::Measure, now);
        }
        if reconcile {
            self.act(Action::Reconcile, now);
        }
        if let Some(n) = set_cap.and_then(|x| x.parse::<u64>().ok()) {
            if !matches!(self.act(Action::SetCap { bytes: n }, now), Applied::Trouble(_)) {
                self.ux.cap_edit.land();
            }
        }
        if recheck {
            self.act(Action::RecheckAll, now);
        }
    }

    pub(super) fn dir_open(&mut self, d: DirSheet) {
        let docs = self.shell.documents.clone();
        let slot = self.dir_slot(d);
        if slot.trim().is_empty() {
            *slot = docs;
        }
        if d == DirSheet::Migrate {
            self.dir_landing();
        }
        self.ux.dir_sheet = Some(d);
    }

    fn dir_slot(&mut self, d: DirSheet) -> &mut String {
        match d {
            DirSheet::Home => &mut self.typed.home,
            DirSheet::GrantDir => &mut self.typed.grant_dir,
            DirSheet::Migrate => &mut self.typed.migrate,
            DirSheet::Adopt => &mut self.typed.adopt,
        }
    }

    fn dir_landing(&mut self) {
        self.ux.migrate_landing = None;
    }

    pub(super) fn dir_sheet(&mut self, ctx: &egui::Context, now: f64) {
        let Some(d) = self.ux.dir_sheet else { return };
        let moving = self.shell.tasks.in_flight(crate::task::Kind::Migrate);
        if d == DirSheet::Migrate && self.ux.dir_waiting && !moving {
            self.ux.dir_waiting = false;
            self.ux.dir_sheet = None;
            return;
        }
        let title = match d {
            DirSheet::Home => Key::SetChangeHome,
            DirSheet::GrantDir => Key::V2ImportGrantDir,
            DirSheet::Migrate => Key::MigrateLabel,
            DirSheet::Adopt => Key::AdoptGroup,
        };
        let at = self.dir_slot(d).clone();
        if d == DirSheet::Migrate && !at.trim().is_empty() {
            if let Some(crate::peekx::Got::Landing(c)) = self.peek(crate::peekx::Ask::Landing { dir: at.trim().to_string() }, now) {
                self.ux.migrate_landing = Some(c);
            }
        }
        let landing = self.ux.migrate_landing.clone().filter(|_| d == DirSheet::Migrate);
        let ready = match d {
            DirSheet::Home => Self::landing_ok(&at),
            DirSheet::Migrate => landing.as_ref().is_some_and(|p| Self::landing_ok(&p.at.display().to_string())),
            _ => !at.trim().is_empty(),
        };
        let phase = if d == DirSheet::Migrate { self.phase_of(crate::task::Kind::Migrate) } else { Phase::Idle };
        let (mut go, mut close, mut picked) = (false, false, None);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("data-folder", tk::SHEET_W),
            self,
            |ui, me| {
                sheet::title(ui, t(title), "");
                let entries = me.shell.archive_now().map(|a| a.items.to_string());
                card::form(ui, |ui, f| {
                    if let (DirSheet::Migrate, Some(n)) = (d, entries.as_deref()) {
                        let _ = f.info(ui, t(Key::LedgerEntries), card::Show::Text(n));
                    }
                    let label = match d {
                        DirSheet::Home => t(Key::SetHomeOfSeat),
                        DirSheet::GrantDir => t(Key::Folder),
                        DirSheet::Migrate => t(Key::MoveTo),
                        DirSheet::Adopt => t(Key::WizAdoptFolder),
                    };
                    f.folder(ui, label, &at, t(Key::PickNone));
                });
                if let Some(k) = landing.as_ref().and_then(|p| p.why.say_ahead().map(|k| (k, p))) {
                    hint(ui, &fill1(k.0, &width::file_name(&k.1.at.display().to_string())));
                }
            },
            |ui, _me| {
                let role = if matches!(d, DirSheet::Migrate | DirSheet::Adopt) { Role::Commit } else { Role::Primary };
                let k = match d {
                    DirSheet::Home => key::Key::new(t(Key::DoOpenHome), role),
                    DirSheet::GrantDir => key::Key::new(t(Key::V2ImportGrantDirGo), role),
                    DirSheet::Migrate => key::Key::new(t(Key::DoMigrate), role),
                    DirSheet::Adopt => key::Key::new(t(Key::DoAdopt), role),
                };
                go = key::show(ui, k.enabled(ready).phase(phase).busy_text(t(Key::SetMigrating))).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                picked = foot_pick(ui, "data-folder", crate::platform::Pick::Folder);
            },
        );
        if let Some(p) = picked {
            *self.dir_slot(d) = p;
            if d == DirSheet::Migrate {
                self.dir_landing();
            }
        }
        if close || out.esc {
            self.ux.dir_sheet = None;
            return;
        }
        if !go || moving {
            return;
        }
        match d {
            DirSheet::Home => {
                let a = Action::ChangeHome { root: self.typed.home.clone() };
                self.act(a, now);
                self.ux.lang_applied = false;
                self.ux.dir_sheet = None;
            }
            DirSheet::GrantDir => {
                let a = Action::ImportGrantDir { dir: self.typed.grant_dir.clone() };
                self.act(a, now);
                self.ux.dir_sheet = None;
            }
            DirSheet::Adopt => {
                let a = Action::Adopt { dir: self.typed.adopt.clone() };
                self.act(a, now);
                self.ux.dir_sheet = None;
            }
            DirSheet::Migrate => {
                if let Some(p) = self.ux.migrate_landing.clone() {
                    let a = Action::MigrateHome { to: p.at.display().to_string() };
                    if matches!(self.act(a, now), Applied::Started(crate::task::Kind::Migrate)) {
                        self.ux.dir_waiting = true;
                    } else {
                        self.ux.dir_sheet = None;
                    }
                    self.typed.migrate.clear();
                    self.ux.migrate_landing = None;
                }
            }
        }
    }

    pub(super) fn backup_behind(&self) -> Option<u64> {
        crate::machine::backup_behind(self.shell.machine.backup.as_ref(), self.shell.items_now)
    }

    pub(super) fn backup_point(&self) -> (Mark, String) {
        let last = self.shell.machine.backup.as_ref();
        match (crate::firstrun::backup_point(last, self.shell.items_now, self.shell.machine.backup_failed), last) {
            (crate::firstrun::Shade::Red, _) => (Mark::Bad, t(Key::GapBackupFailed).to_string()),
            (crate::firstrun::Shade::Amber, _) => (Mark::Warn, fill1(Key::GapBackupBehind, &self.backup_behind().unwrap_or(0).to_string())),
            (crate::firstrun::Shade::Green, Some(b)) => (Mark::Ok, crate::when::when(b.at)),
            (crate::firstrun::Shade::Grey, Some(b)) => (Mark::Todo, crate::when::when(b.at)),
            _ => (Mark::Todo, t(Key::GapBackupNever).to_string()),
        }
    }

    fn set_about(&mut self, ui: &mut egui::Ui, now: f64) {
        let unseated = self.shell.seat_unseated();
        let last = self.shell.last.as_ref().map(|x| format!("{} \u{b7} {} \u{b7} {}", x.found(), x.faces.iter().map(|f| f.bytes).sum::<u64>(), x.marks));
        let phase = self.phase_of(crate::task::Kind::SelfCheck);
        let title = format!("{} {}", t(Key::AppName), crate::about::VERSION);
        stagger(ui, 0, |ui| {
            card::form(ui, |ui, f| f.summary(ui, Glyph::Info, card::Tint::Blue, &title, None, build_kind(), |_| ()));
        });
        let points = self.checklist();
        let goes: Vec<Point> = match self.shell.settings.role {
            crate::roles::Role::Author => Author::ALL
                .iter()
                .map(|a| match a {
                    Author::AnchorKey => Point::Step(crate::nav::Step::Key),
                    Author::Pin => Point::Step(crate::nav::Step::Pin),
                    Author::GasFloat => Point::Step(crate::nav::Step::Gas),
                    Author::Genesis => Point::Step(crate::nav::Step::Genesis),
                    Author::Backup => Point::Backup,
                })
                .collect(),
            crate::roles::Role::Grantee => Grantee::ALL
                .iter()
                .map(|g| match g {
                    Grantee::AnchorKey => Point::Step(crate::nav::Step::Key),
                    Grantee::Pin => Point::Step(crate::nav::Step::Pin),
                    Grantee::Endpoints => Point::Network,
                    Grantee::Ready => Point::Step(crate::nav::Step::Gas),
                    Grantee::Backup => Point::Backup,
                })
                .collect(),
        };
        let marked = self.marked_title();
        if self.shell.settings.role == crate::roles::Role::Author && !self.shell.rooted && marked.is_none() {
            self.genesis_offered();
        }
        let mut go = None;
        let (mut again, mut self_check) = (false, false);
        stagger(ui, 1, |ui| {
            card::section(ui, t(Key::SetMissing), "", |ui| {
                card::form(ui, |ui, f| {
                    for ((m, name, _), p) in points.iter().zip(&goes) {
                        if *m == Mark::Ok {
                            let word = match p {
                                Point::Step(crate::nav::Step::Genesis) | Point::Backup => t(Key::Done),
                                Point::Step(crate::nav::Step::Gas) | Point::Network => t(Key::PointReady),
                                _ => "",
                            };
                            let _ = f.info(ui, name, card::Show::Mark(Mark::Ok, word));
                            continue;
                        }
                        let fetch = matches!(p, Point::Step(crate::nav::Step::Genesis)) && marked.is_some();
                        let label = match p {
                            _ if fetch => t(Key::DoFetchLedger),
                            Point::Fetch => t(Key::DoFetchLedger),
                            Point::Step(s) => t(s.title()),
                            Point::Backup => t(Key::BackupExportTitle),
                            Point::Network => t(Key::SetEditNodes),
                        };
                        let name = if fetch { marked.as_deref().unwrap_or(name) } else { name };
                        if f.act(ui, name, "", key::Key::new(label, Role::Secondary).enabled(!unseated)).clicked() {
                            go = Some(if fetch { Point::Fetch } else { *p });
                        }
                    }
                });
            });
        });
        stagger(ui, 2, |ui| {
            card::form(ui, |ui, f| {
                again = f.act(ui, t(Key::PageFirstRun), "", key::Key::new(t(Key::DoWizardAgain), Role::Secondary).enabled(!unseated)).clicked();
                self_check = f.act(ui, t(Key::TaskSelfCheck), last.as_deref().unwrap_or(""), key::Key::new(t(Key::DoSelfCheck), Role::Secondary).phase(phase)).clicked();
            });
            self.stage_line(ui, crate::task::Kind::SelfCheck);
        });
        if again {
            let step = self.progress().first_open().unwrap_or(crate::nav::Step::Pin);
            self.wizard_open(step);
        }
        match go {
            Some(Point::Step(s)) => self.wizard_open(s),
            Some(Point::Backup) => self.bk_open(Bk::Export),
            Some(Point::Network) => self.go(Place::Settings(Section::Network), now),
            Some(Point::Fetch) => self.go(Place::Settings(Section::Data), now),
            None => {}
        }
        let fonts: Vec<String> = zikaron_ui::fonts::Role::ALL
            .iter()
            .map(|role| match self.shell.fonts.face(*role) {
                Some(f) => {
                    let place = t(if f.place == zikaron_ui::fonts::Place::Embedded { Key::FontEmbedded } else { Key::FontFromSystem });
                    match f.licence() {
                        Some(l) => format!("{} {} \u{b7} {} \u{b7} {place} \u{b7} {l}", role.as_str(), f.file, f.index),
                        None => format!("{} {} \u{b7} {} \u{b7} {place}", role.as_str(), f.file, f.index),
                    }
                }
                None => format!("{} {}", role.as_str(), t(Key::Missing)),
            })
            .collect();
        let sink = match trace::sink() {
            Sink::Memory => t(Key::TraceMemory).to_string(),
            Sink::File(p) => width::file_name(&p.display().to_string()),
        };
        let flying = self.shell.tasks.flying();
        let rows = [
            (t(Key::SetCores), Val::mono(crate::sign::domains().join(" \u{b7} "))),
            (t(Key::BuildKind), Val::figure(build_kind().to_string())),
            (t(Key::FontGroup), Val::figure(fonts.join("\n"))),
            (t(Key::TraceSink), Val::mono(sink)),
            (t(Key::TaskInFlight), if flying.is_empty() { Val::text(t(Key::Nothing)) } else { Val::figure(flying.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(" ")) }),
        ];
        stagger(ui, 2, |ui| details_card(ui, "about-details", &rows));
        stagger(ui, 3, |ui| {
            card::form(ui, |ui, f| {
                let _ = f.fold(ui, "about-notices", t(Key::SetNotices), &crate::about::count().to_string(), |ui, f| {
                    f.free(ui, |ui| {
                        let lines = crate::about::notice_lines();
                        let row_h = Type::MonoSmall.line();
                        ui.spacing_mut().item_spacing.y = 0.0;
                        zikaron_ui::scroll::nested_rows(ui, egui::ScrollArea::both().id_salt("about-notices-text").max_height(320.0).auto_shrink([false, true]), row_h, lines.len(), |ui, range| {
                            for l in &lines[range] {
                                ui.add(egui::Label::new(egui::RichText::new(*l).font(Type::MonoSmall.font()).color(c(C::Ink2)).line_height(Some(row_h))).wrap_mode(egui::TextWrapMode::Extend));
                            }
                        });
                    })
                });
            });
        });
        if self_check {
            self.act(Action::SelfCheck, now);
        }
    }

    pub(super) fn checklist(&self) -> Vec<(Mark, &'static str, String)> {
        let key_mark = if self.shell.anchor.is_some() { Mark::Ok } else { Mark::Bad };
        let pin_mark = if self.shell.vault.absent() { Mark::Bad } else { Mark::Ok };
        let gas = self.gas_mark();
        let (backup, backup_say) = self.backup_point();
        match self.shell.settings.role {
            crate::roles::Role::Author => Author::ALL
                .iter()
                .map(|a| match a {
                    Author::AnchorKey => (key_mark, t(Key::PointAnchorKey), t(Key::GuideMakeKey).to_string()),
                    Author::Pin => (pin_mark, t(Key::IdPin), t(Key::GuideSetPin).to_string()),
                    Author::GasFloat => (gas.0, t(Key::PointGasFloat), gas.1.to_string()),
                    Author::Genesis => (if self.shell.rooted { Mark::Ok } else { Mark::Bad }, t(Key::WizGenesisTitle), t(Key::GuideGenesis).to_string()),
                    Author::Backup => (backup, t(Key::WizBackupTitle), backup_say.clone()),
                })
                .collect(),
            crate::roles::Role::Grantee => Grantee::ALL
                .iter()
                .map(|g| match g {
                    Grantee::AnchorKey => (key_mark, t(Key::PointAnchorKey), t(Key::GuideMakeKey).to_string()),
                    Grantee::Pin => (pin_mark, t(Key::IdPin), t(Key::GuideSetPin).to_string()),
                    Grantee::Endpoints => (if self.shell.endpoints.is_empty() { Mark::Bad } else { Mark::Ok }, t(Key::PointEndpoints), t(Key::GuideEndpoints).to_string()),
                    Grantee::Ready => (gas.0, t(Key::PointReady), gas.1.to_string()),
                    Grantee::Backup => (backup, t(Key::WizBackupTitle), backup_say.clone()),
                })
                .collect(),
        }
    }

    pub(super) fn gas_mark(&self) -> (Mark, &'static str) {
        if self.shell.endpoints.is_empty() {
            return (Mark::Todo, t(Key::NoEndpointYet));
        }
        match self.chain_read() {
            Ok(Some(Done::Chain { gas_wei: Some(w), .. })) if *w > 0 => (Mark::Ok, t(Key::Done)),
            Ok(Some(Done::Chain { gas_wei: Some(_), .. })) => (Mark::Bad, t(Key::GuideGas)),
            _ => (Mark::Todo, t(Key::GuideGas)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Point {
    Step(crate::nav::Step),
    Backup,
    Network,
    Fetch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DirSheet {
    Home,
    GrantDir,
    Migrate,
    Adopt,
}

#[derive(Default)]
pub(super) struct ReadNetUx {
    sheet: Option<(Option<(u64, crate::key::Address)>, Draft)>,
    armed: bool,
}

#[derive(Clone, Default)]
pub(super) struct Draft {
    preset: Option<usize>,
    name: String,
    chain: String,
    registry: String,
    from: String,
    nodes: String,
}

pub(super) fn preset_menu(ui: &mut egui::Ui, salt: &str, at: Option<usize>) -> Option<usize> {
    let labels: Vec<&str> = crate::deploy::KNOWN.iter().map(|d| t(d.label)).collect();
    let items: Vec<menu::Item> = labels.iter().enumerate().map(|(i, l)| menu::Item::Row(menu::Row { label: l, check: Some(at == Some(i)), ..Default::default() })).collect();
    menu::menu_key(ui, salt, at.map(|i| labels[i]).unwrap_or(t(Key::ReadNetPreset)), false, 160.0, &items)
}

fn preset_inner(ui: &mut egui::Ui, salt: &str, at: Option<usize>) -> Option<usize> {
    let labels: Vec<&str> = crate::deploy::KNOWN.iter().map(|d| t(d.label)).collect();
    let items: Vec<menu::Item> = labels.iter().enumerate().map(|(i, l)| menu::Item::Row(menu::Row { label: l, check: Some(at == Some(i)), ..Default::default() })).collect();
    menu::inner_menu_key(ui, salt, at.map(|i| labels[i]).unwrap_or(t(Key::ReadNetPreset)), 160.0, &items)
}

impl Draft {
    fn of(n: &crate::readnets::Net) -> Draft {
        Draft {
            preset: None,
            name: n.name.clone().unwrap_or_default(),
            chain: n.chain_id.to_string(),
            registry: n.registry.hex(),
            from: n.from_block.to_string(),
            nodes: n.nodes.join("\n"),
        }
    }

    fn missing(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.nodes.trim().is_empty() {
            v.push(t(Key::SetNodes));
        }
        if self.chain.trim().parse::<u64>().is_err() {
            v.push(t(Key::BasisChain));
        }
        if self.from.trim().parse::<u64>().is_err() {
            v.push(t(Key::BasisFrom));
        }
        if crate::key::Address::parse(&self.registry).is_none() {
            v.push(t(Key::SetRegistry));
        }
        v
    }

    fn save(&self, was: Option<(u64, crate::key::Address)>) -> Action {
        Action::SaveReadNetwork {
            was: was.map(|(c, r)| (c, r.hex())),
            name: self.name.clone(),
            chain: self.chain.clone(),
            registry: self.registry.clone(),
            from_block: self.from.clone(),
            nodes: self.nodes.clone(),
        }
    }
}

fn read_tone(r: crate::widex::Reading) -> PillTone {
    use crate::widex::Reading as R;
    match r {
        R::Agreed(_) => PillTone::Ok,
        R::Single => PillTone::Warn,
        R::Down | R::Fingerprint => PillTone::Bad,
    }
}

fn registry_still(registry: &str, missing: &[&str]) -> String {
    if crate::addressedx::not_address(registry, crate::addressedx::Takes::Address) {
        t(Key::KitToWhyAddress).to_string()
    } else {
        fill1(Key::GrantStillNeed, &missing.join(t(Key::ListJoin)))
    }
}
fn net_cells(ui: &mut egui::Ui, d: &mut Draft) {
    let known = d.chain.trim().parse::<u64>().ok().and_then(crate::readnets::known_name);
    let mut preset = None;
    let mono = fields::Look { mono: true, ..Default::default() };
    fields::stack(ui, |ui, st| {
        st.field(ui, t(Key::ReadNetName), None, None, |ui, w| {
            let mut fixed = known.map(|k| t(k).to_string());
            let text = match fixed.as_mut() {
                Some(f) => f,
                None => &mut d.name,
            };
            let look = fields::Look { read_only: known.is_some(), ..Default::default() };
            let _ = fields::joined(ui, text, "", w, look, Some(|ui: &mut egui::Ui| preset = preset_inner(ui, "readnet-preset", d.preset)));
        });
        st.field(ui, t(Key::SetNodes), None, None, |ui, _| input::area_hint(ui, &mut d.nodes, 2, t(Key::OnePerLine)));
        st.pair(
            ui,
            |ui, st| st.field(ui, t(Key::BasisChain), None, None, |ui, w| fields::line(ui, &mut d.chain, "", w, mono)),
            |ui, st| st.field(ui, t(Key::BasisFrom), None, None, |ui, w| fields::line(ui, &mut d.from, "", w, mono)),
        );
        let reg_bad = crate::addressedx::not_address(&d.registry, crate::addressedx::Takes::Address);
        st.field(ui, t(Key::SetRegistry), None, reg_bad.then(|| (t(Key::KitToWhyAddress), true)), |ui, w| fields::line(ui, &mut d.registry, "", w, fields::Look { bad: reg_bad, ..mono }));
    });
    if let Some(i) = preset {
        let c = crate::deploy::KNOWN[i].cells();
        d.preset = Some(i);
        d.chain = c.chain;
        d.registry = c.registry;
        d.from = c.from;
        d.nodes = c.nodes;
    }
}

const CLI_SPAN: [(Key, u64, u64); 3] = [(Key::CliLastsDays, 365, 86_400), (Key::CliLastsHours, 23, 3_600), (Key::CliLastsMinutes, 59, 60)];
pub(super) const CLI_SPAN_NUM_W: f32 = zikaron_ui::lasts::NUM_W;
pub(super) const CLI_SPAN_GAP: f32 = zikaron_ui::lasts::GAP;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Span {
    Empty,
    Over([bool; 3]),
    Zero,
    PastYear,
    Lasts(crate::machine::Lasts),
}

fn cli_span(cells: &[String; 3], never: bool) -> Span {
    use crate::machine::{Lasts, LASTS_MAX};
    use zikaron_ui::lasts::Read;
    if never {
        return Span::Lasts(Lasts::Never);
    }
    let parts = CLI_SPAN.map(|(_, most, unit)| (most, unit));
    match zikaron_ui::lasts::read(cells, &parts) {
        Read::Empty => Span::Empty,
        Read::Over(over) => Span::Over(std::array::from_fn(|i| over.get(i).copied().unwrap_or(false))),
        Read::Total(0) => Span::Zero,
        Read::Total(s) if s > LASTS_MAX => Span::PastYear,
        Read::Total(s) => Span::Lasts(Lasts::Secs(s)),
    }
}

fn cli_span_said(span: Span, now: u64) -> (String, bool) {
    use crate::machine::Lasts;
    match span {
        Span::Empty | Span::Lasts(Lasts::Never) => (String::new(), false),
        Span::Over(over) => {
            let each: Vec<String> = CLI_SPAN.iter().zip(over).filter(|(_, o)| *o).map(|((unit, most, _), _)| crate::lang::filln(Key::CliLastsAtMost, &[t(*unit), &most.to_string()])).collect();
            (each.join(t(Key::ListJoin)), true)
        }
        Span::Zero => (t(Key::CliLastsZero).to_string(), true),
        Span::PastYear => (t(Key::CliLastsPastYear).to_string(), true),
        Span::Lasts(Lasts::Secs(s)) => (cli_expiry_said(Some(now + s)), false),
    }
}

pub(super) fn cli_expiry_said(expires: Option<u64>) -> String {
    match expires {
        Some(at) => fill1(Key::CliAloneUntil, &crate::when::when(at)),
        None => t(Key::CliLastsNever).to_string(),
    }
}

#[cfg(test)]
mod cli_lasts_tests {
    use super::*;

    fn cells(d: &str, h: &str, m: &str) -> [String; 3] {
        [d.to_string(), h.to_string(), m.to_string()]
    }

    #[test]
    fn a_validity_is_three_cells_within_a_minute_and_a_year_or_never() {
        use crate::machine::Lasts;
        assert_eq!(cli_span(&cells("", "", ""), false), Span::Empty);
        assert_eq!(cli_span(&cells("", "", "1"), false), Span::Lasts(Lasts::Secs(60)));
        assert_eq!(cli_span(&cells("1", "2", "3"), false), Span::Lasts(Lasts::Secs(86_400 + 7_200 + 180)));
        assert_eq!(cli_span(&cells("365", "", ""), false), Span::Lasts(Lasts::Secs(365 * 86_400)));
        assert_eq!(cli_span(&cells("365", "", "1"), false), Span::PastYear);
        assert_eq!(cli_span(&cells("0", "0", "0"), false), Span::Zero);
        assert_eq!(cli_span(&cells("366", "24", "59"), false), Span::Over([true, true, false]));
        assert_eq!(cli_span(&cells("", "", "99999999999999999999999"), false), Span::Over([false, false, true]));
        assert_eq!(cli_span(&cells("400", "x", ""), true), Span::Lasts(Lasts::Never));
        for s in [cli_span(&cells("1", "", ""), false), cli_span(&cells("", "", ""), true)] {
            if let Span::Lasts(l) = s {
                assert!(l.ok());
            }
        }
    }

    #[test]
    fn a_cell_keeps_its_digits() {
        assert_eq!(zikaron_ui::lasts::digits("0012"), "12");
        assert_eq!(zikaron_ui::lasts::digits("1a2"), "12");
        assert_eq!(zikaron_ui::lasts::digits("000"), "0");
        assert_eq!(zikaron_ui::lasts::digits("x"), "");
    }

    #[test]
    fn the_line_under_the_switch_says_the_expiry_or_why_not() {
        use crate::machine::Lasts;
        assert_eq!(cli_span_said(Span::Lasts(Lasts::Secs(60)), 1_000), (cli_expiry_said(Some(1_060)), false));
        assert_eq!(cli_span_said(Span::Zero, 0), (t(Key::CliLastsZero).to_string(), true));
        assert_eq!(cli_span_said(Span::PastYear, 0), (t(Key::CliLastsPastYear).to_string(), true));
        let over = cli_span_said(Span::Over([true, false, true]), 0);
        assert!(over.1 && over.0.contains("365") && over.0.contains("59") && !over.0.contains("23"), "{over:?}");
        assert_eq!(cli_span_said(Span::Empty, 0), (String::new(), false));
        assert_eq!(cli_span_said(Span::Lasts(Lasts::Never), 0), (String::new(), false));
    }
}

impl Win {
    pub(super) fn clean_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if !self.ux.clean_sheet {
            return;
        }
        let busy = self.shell.tasks.in_flight(crate::task::Kind::CleanList) || self.shell.tasks.in_flight(crate::task::Kind::Clean);
        let got = self.shell.cleaned.clone();
        let done = got.as_ref().is_some_and(|(_, d)| *d);
        let plan = got.map(|(p, _)| p).unwrap_or_default();
        let ready = !busy && !done && !plan.remove.is_empty();
        let kind_word = |k: zikaron_seal::exports::Kind| match k {
            zikaron_seal::exports::Kind::Kit => t(Key::PageKit),
            zikaron_seal::exports::Kind::GrantPackage => t(Key::KindGrantPackage),
            zikaron_seal::exports::Kind::Mirror => t(Key::PageMirror),
            zikaron_seal::exports::Kind::Proofs => t(Key::CleanProofs),
        };
        let (mut go, mut close) = (false, false);
        let phase = self.phase_of(crate::task::Kind::Clean);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("clean-exports", tk::SHEET_W),
            self,
            |ui, _me| {
                sheet::title(ui, t(Key::CleanTitle), "");
                if busy && plan.remove.is_empty() && plan.keep.is_empty() {
                    states::skeleton_lines(ui, &[0.5, 0.7]);
                    return;
                }
                card::form(ui, |ui, f| {
                    if plan.remove.is_empty() && plan.keep.is_empty() {
                        let _ = f.info(ui, t(Key::CleanNone), card::Show::Empty(""));
                        return;
                    }
                    if done {
                        f.note(ui, &crate::lang::filln(Key::SaidCleaned, &[&plan.remove.len().to_string(), &plan.keep.len().to_string()]));
                    } else {
                        let _ = f.info(ui, t(Key::CleanWill), card::Show::Text(&fill1(Key::CleanCount, &plan.remove.len().to_string())));
                    }
                    for i in &plan.remove {
                        let sub = if i.link.is_some() { format!("{} \u{b7} {}", kind_word(i.kind), t(Key::CleanPublished)) } else { kind_word(i.kind).to_string() };
                        let _ = f.line(ui, None, &width::file_name(&i.path), &sub, false, false, c(C::Ink), |_| ());
                    }
                    for (i, why) in &plan.keep {
                        let why = match why {
                            zikaron_seal::exports::Why::Changed => t(Key::CleanChanged),
                            zikaron_seal::exports::Why::Missing => t(Key::CleanMissing),
                            zikaron_seal::exports::Why::Unremoved(_) => t(Key::CleanUnremoved),
                            zikaron_seal::exports::Why::Foreign => t(Key::CleanForeign),
                        };
                        let _ = f.info(ui, &width::file_name(&i.path), card::Show::Mark(Mark::Warn, why));
                    }
                    if done {
                        f.note(ui, t(Key::CleanEarlier));
                    }
                });
            },
            |ui, _me| {
                if !done {
                    go = key::show(ui, key::Key::new(t(Key::V2DeleteGo), Role::Danger).enabled(ready).phase(phase)).clicked();
                }
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
            },
        );
        if close || out.esc {
            self.ux.clean_sheet = false;
            return;
        }
        if go {
            self.act(Action::CleanExports { yes: crate::exports::YES.to_string() }, now);
        }
    }
}
