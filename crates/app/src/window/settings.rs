//! Settings: the home (one line per section: its glyph, its name, a chevron) and the section pages.
//! Technical values (addresses, nodes, contract, chain id, start block, paths, key file names, the instance
//! lock) live under each section's details.

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
        // The time zone's line says the time as it is shown, or why the system's zone is not read.
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

    /// The command line's page, three groups: the terminal command; the automation level (the two ways it may
    /// write this machine's data); how its anchoring is handled, for both ways. Each switch keeps its behavior and
    /// where it is stored; the page only gathers them.
    fn set_cli(&mut self, ui: &mut egui::Ui, now: f64) {
        self.set_cli_path(ui, now);
        self.set_cli_writes(ui, now);
        self.set_cli_anchor(ui, now);
    }

    /// "Putting on chain from the command line" (machine-wide): sent as the send key sends, or left queued for
    /// the person; one judgement for both of the command line's ways (`zikaron_seal::machine::turn`).
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

    /// "Enable the zikaron command": a switch with a line under it only when there is something to say
    /// (installed, taken by another install, provided by a package, or not supported here, with the reason
    /// folded). Its state is reread whenever the row is shown again after a pause (by the action layer, never the
    /// frame); when it cannot be installed, the switch does not flip.
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
        // "Connect an AI app": the config that starts the command line's MCP server, copied; grey where the command
        // line cannot be reached from outside the app (a disk image opened in place, an AppImage), and why on hover.
        let config = self.shell.ai_config.clone();
        let mut flip = false;
        let mut copy = false;
        stagger(ui, 0, |ui| {
            card::section(ui, t(Key::SetCliTerminal), "", |ui| {
                card::form(ui, |ui, f| {
                    flip = f.switch(ui, t(Key::SetCliPath), &say, on, can && !busy);
                    // Why it cannot be installed: the details, folded in the same card.
                    if let State::Unsupported(why) = &state {
                        f.fold(ui, "cli-path-why", t(Key::SetEvidence), "", |ui, f| f.note(ui, why));
                    }
                    copy = f.with(ui, t(Key::SetCliAiApp), "", |ui| {
                        let k = key::Key::new(t(Key::DoCopyAiConfig), Role::Secondary).enabled(config.is_some());
                        key::show(ui, if config.is_some() { k } else { k.why(t(Key::CliPathUnsupportedSay)) }).clicked()
                    });
                });
            });
        });
        if flip {
            self.act(Action::SetCliPath { on: !on }, now);
        }
        if let Some(c) = config.filter(|_| copy) {
            ui.ctx().copy_text(c);
            self.toasts.say(t(Key::SaidCopiedAiConfig), Tone::Note, now);
        }
    }

    /// The command line's two ways to write this machine's sealed data, one group ("automation level"): "through
    /// the desktop app" (requests to the running app; no passcode) and "with a credential" (the passcode and a
    /// validity each time it is turned on, on its sheet). Writing alone outranks the desktop path: while it is on, the
    /// desktop switch is greyed out and the desktop accepts no command-line requests (`machine::Machine::cli_path`).
    fn set_cli_writes(&mut self, ui: &mut egui::Ui, now: f64) {
        // The two switches' four states are resolved in one place (`cli_path`); the page only draws them.
        let path = self.shell.machine.cli_path();
        let alone = path == crate::machine::CliPath::Alone;
        let desktop = path == crate::machine::CliPath::Desktop;
        let unlocked = self.shell.unlocked();
        // The second lines say state only: when the credential ends, and why the desktop switch is greyed out.
        let alone_say = match (alone, self.shell.credential) {
            (true, Some(c)) => cli_expiry_said(c.expires),
            _ => String::new(),
        };
        let desktop_say = if alone { t(Key::SetCliDesktopGrey) } else { "" };
        let (mut flip_desktop, mut flip_alone) = (false, false);
        stagger(ui, 1, |ui| {
            card::section(ui, t(Key::SetCliLevel), "", |ui| {
                card::form(ui, |ui, f| {
                    flip_desktop = f.switch(ui, t(Key::SetCliDesktop), desktop_say, desktop, unlocked && !alone);
                    flip_alone = f.switch(ui, t(Key::SetCliAlone), &alone_say, alone, unlocked);
                });
            });
        });
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

    /// Go to the command line's switches (its own settings page), from a toast's key.
    pub(super) fn go_cli_switches(&mut self, now: f64) {
        self.go(Place::Settings(Section::Cli), now);
    }

    /// Open the "write alone" sheet (every other sheet closes); the validity starts empty.
    pub(super) fn cli_open(&mut self) {
        self.sheets_clear();
        self.ux.cli_sheet = true;
        self.ux.cli_span = Default::default();
        self.ux.cli_never = false;
    }

    /// Close the "write alone" sheet, wiping the passcode typed on it.
    pub(super) fn cli_close(&mut self) {
        self.ux.cli_sheet = false;
        self.ux.cli_pin.clear();
        self.ux.cli_trouble = None;
    }

    /// The "write alone" sheet: that the command line may spend gas, in a note; how long the credential lasts
    /// in one row: days, hours and minutes in one box (an empty cell is zero), or never by the switch on the
    /// row's head, which leaves the box faint and not taken; then the passcode. A cell past its most is red and
    /// the line under the box says why; the key turns it on only with a whole passcode and a validity in range.
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
            sheet::Spec::new("cli-alone", tk::SHEET_W),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::SetCliAlone), "");
                fields::stack(ui, |ui, st| {
                    // The validity: its name, then "does not expire" and its switch right after it on the same line;
                    // days, hours and minutes in one box under it (faint while it never expires); the line under the
                    // box takes room only while it has words, opening and closing with them.
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = tk::S2;
                        ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), tk::KEY_H), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = tk::S2;
                            // A field's name is set in the key face (14, medium).
                            let named = ui.painter().layout_no_wrap(t(Key::U3Window).to_string(), key::key_font(), c(C::Ink)).size().x;
                            ui.allocate_ui(egui::vec2(named, Type::Note.line()), |ui| fields::name(ui, t(Key::U3Window), None));
                            let w = ui.add(egui::Label::new(egui::RichText::new(t(Key::CliLastsNever)).font(Type::Note.font()).color(c(C::Ink2))).selectable(false).sense(egui::Sense::click()));
                            let s = toggle::switch(ui, me.ux.cli_never, true);
                            if s.clicked() || w.clicked() {
                                me.ux.cli_never = !me.ux.cli_never;
                            }
                        });
                        let never = me.ux.cli_never;
                        let units: Vec<&str> = CLI_SPAN.iter().map(|(unit, _, _)| t(*unit)).collect();
                        let bad: Vec<bool> = (0..CLI_SPAN.len()).map(|i| matches!(span, Span::Over(o) if o[i])).collect();
                        zikaron_ui::lasts::boxed(ui, "cli-span", &mut me.ux.cli_span, &units, "0", &bad, !never);
                        ui.spacing_mut().item_spacing.y = 0.0;
                        passing_line(ui, egui::Id::new("cli-span-said"), &said.0, said.1);
                    });
                    let _ = pin_field(st, ui, t(Key::IdPinGate), None, "cli-alone-pin", &mut me.ux.cli_pin, me.ux.pin_shake, !busy);
                });
                if let Some(f) = &trouble {
                    states::err_box(ui, "cli-trouble", f.human(), &f.next(), t(Key::U3RawError), &f.raw());
                }
            },
            |ui, _me| {
                go = page::Page::new().primary_with(ui, t(Key::DoCliAloneOn), pin_full && lasts.is_some() && !busy).1.clicked();
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

    fn set_keys(&mut self, ui: &mut egui::Ui, now: f64) {
        let seat = self.shell.settings.role;
        let reg = self.shell.identities.clone().unwrap_or_default();
        let current = reg.now().map(|(r, s)| (r.clone(), s));
        let unseated = self.shell.seat_unseated();
        let has_words = current.as_ref().map(|(r, _)| r.kind() == crate::identity::Kind::Words).unwrap_or(false);
        let can_sign = !unseated && self.shell.anchor.is_some();
        let name = current.as_ref().map(|(r, _)| if r.label.trim().is_empty() { t(Key::Unnamed).to_string() } else { r.label.clone() }).unwrap_or_else(|| t(Key::IdNone).to_string());
        let kind = current.as_ref().map(|(r, _)| format!("{} \u{b7} {}", t(id_seat_key(seat)), t(id_kind_key(r.kind())))).unwrap_or_default();
        if self.shell.tasks.in_flight(crate::task::Kind::Keystore) && !self.status_reports(crate::task::Kind::Keystore) {
            states::note_box(ui, t(Key::V2Encrypting));
        }
        if unseated {
            zikaron_ui::callout::callout(ui, t(Key::IdSeatEmptyNote));
        }
        // The overview: the identity's name and kind, its address (copied from its row), balance, backup
        // state, when it was made and where it is kept (the action layer reads the disk; the frame reads fields).
        let created = match current.as_ref().map(|(r, _)| r.created.clone()) {
            Some(ref c) if c != crate::identity::NO_CREATED => c.get(0..10).unwrap_or(c).to_string(),
            _ => t(Key::IdCreatedNone).to_string(),
        };
        let addr = self.shell.anchor.filter(|_| !unseated).map(|a| a.hex());
        let gas = match self.chain_read() {
            Err(f) => (Mark::Bad, fill1(Key::SetReadFailedNow, f.human())),
            Ok(Some(Done::Chain { gas_wei: Some(w), .. })) => (if *w > 0 { Mark::Ok } else { Mark::Bad }, fill1(Key::SetGasSay, &eth_held(*w))),
            _ => (Mark::Todo, t(Key::SetNotRead).to_string()),
        };
        let backed = match &current {
            Some((r, _)) => {
                let k = match (r.backed_words, r.backed_file) {
                    (true, true) => Key::IdBackedBoth,
                    (true, false) => Key::IdBackedWords,
                    (false, true) => Key::IdBackedFile,
                    (false, false) => Key::IdNotBacked,
                };
                (if r.backed() { Mark::Ok } else { Mark::Warn }, t(k).to_string())
            }
            None => (Mark::Todo, t(Key::SetNotRead).to_string()),
        };
        let seen = self.shell.backup_seen.clone();
        let backup_at = match &seen {
            Some(b) if b.at == crate::identity::NO_BACKUP_AT => (Mark::Todo, t(Key::IdBackupNever).to_string()),
            Some(b) if b.exists && b.opens => (Mark::Ok, t(Key::IdBackupThere).to_string()),
            Some(_) => (Mark::Bad, t(Key::IdBackupGone).to_string()),
            None => (Mark::Todo, t(Key::SetNotRead).to_string()),
        };
        // The primary identity, the only one that recovers the passcode: this one, or another one via "set as
        // primary" (a new master key; everything resealed).
        let primary = self.shell.primary.clone();
        let this_primary = match (&primary, &current) {
            (Some((p, _)), Some((r, _))) => p.eq_ignore_ascii_case(&r.id),
            _ => false,
        };
        let primary_name = primary
            .as_ref()
            .and_then(|(p, _)| reg.find(p))
            .map(|r| if r.label.trim().is_empty() { t(Key::Unnamed).to_string() } else { r.label.clone() })
            .unwrap_or_else(|| t(Key::IdNone).to_string());
        // The overview: the identity's name, its seat and whether it is the primary one, "copy address", and
        // three figures: the address, the balance, the backup state.
        let sub = match &current {
            Some(_) if this_primary => format!("{} \u{b7} {}", t(id_seat_key(seat)), t(Key::IdPrimaryTag)),
            Some(_) => kind.clone(),
            None => String::new(),
        };
        let addr_short = addr.as_deref().map(card::short).unwrap_or_default();
        let addr_fact = match (&addr, unseated) {
            (Some(_), _) => card::Fact::Mono(&addr_short),
            (None, true) => card::Fact::Warn(t(Key::IdSeatEmpty)),
            (None, false) => card::Fact::Warn(t(if current.is_some() { Key::IdKeyMissing } else { Key::SetNoKey })),
        };
        let gas_fact = match gas.0 {
            Mark::Ok => card::Fact::Text(&gas.1),
            Mark::Todo => card::Fact::Quiet(&gas.1),
            _ => card::Fact::Warn(&gas.1),
        };
        // A backup file that is gone, or no longer opens, is flagged here, above the backup state it undoes.
        let backed_fact = if backup_at.0 == Mark::Bad {
            card::Fact::Warn(&backup_at.1)
        } else if backed.0 == Mark::Ok {
            card::Fact::Text(&backed.1)
        } else {
            card::Fact::Warn(&backed.1)
        };
        let mut copy = false;
        stagger(ui, 0, |ui| {
            card::form(ui, |ui, f| {
                copy = f.summary(ui, Glyph::Key, card::Tint::Blue, &name, None, &sub, |ui| addr.is_some() && key::key(ui, t(Key::IdDoCopy), Role::Secondary, true).clicked());
                f.facts(ui, &[(t(Key::IdAddress), addr_fact), (t(Key::IdGas), gas_fact), (t(Key::IdBackupState), backed_fact)]);
            });
        });
        // The passcode group: change passcode, the auto-lock switch, the idle time (only while on). Changes take
        // effect at once; nothing to save.
        let pin_set = !self.shell.vault.absent();
        let lock_on = self.shell.machine.auto_lock;
        let lock_now = self.shell.machine.auto_lock_secs;
        let lock_items: Vec<String> = crate::machine::LOCK_CHOICES.iter().map(|x| fill1(Key::MinutesN, &(x / 60).to_string())).collect();
        let lock_label = fill1(Key::MinutesN, &(lock_now / 60).to_string());
        let (mut change, mut set_pin, mut lock_pick, mut flip) = (false, false, None, false);
        stagger(ui, 1, |ui| {
            card::section(ui, t(Key::IdPin), "", |ui| {
                card::form(ui, |ui, f| {
                    if pin_set {
                        change = f.go(ui, t(Key::ChangePinTitle), "", true, "").clicked();
                        flip = f.switch(ui, t(Key::IdAutoLock), "", lock_on, true);
                        if lock_on {
                            f.with(ui, t(Key::IdIdleFor), "", |ui| {
                                let items: Vec<menu::Item> = lock_items
                                    .iter()
                                    .enumerate()
                                    .map(|(i, s)| menu::Item::Row(menu::Row { label: s, check: Some(crate::machine::LOCK_CHOICES[i] == lock_now), ..Default::default() }))
                                    .collect();
                                lock_pick = menu::menu_key(ui, "auto-lock", &lock_label, false, 150.0, &items);
                            });
                        }
                    } else {
                        set_pin = f.act(ui, t(Key::IdPin), t(Key::IdPinNone), key::Key::new(t(Key::IdDoSetPin), Role::Secondary)).clicked();
                    }
                });
            });
        });
        let (mut backup, mut words) = (false, false);
        stagger(ui, 2, |ui| {
            card::section(ui, t(Key::IdExportGroup), "", |ui| {
                card::form(ui, |ui, f| {
                    backup = f.go(ui, t(Key::IdDoBackupGo), "", can_sign, "").clicked();
                    words = f.go(ui, t(Key::IdWordsTitle), "", !unseated && has_words, if has_words || current.is_none() { "" } else { t(Key::IdNoWordsHint) }).clicked();
                });
            });
        });
        let (mut set_primary, mut switch, mut new, mut import, mut rename) = (false, false, false, false, false);
        stagger(ui, 3, |ui| {
            card::section(ui, t(Key::IdGroup), "", |ui| {
                card::form(ui, |ui, f| {
                    let _ = f.info(ui, t(Key::IdPrimaryTag), card::Show::Text(if this_primary { t(Key::IdPrimaryThis) } else { &primary_name }));
                    if !this_primary {
                        set_primary = f.go(ui, t(Key::PrimaryTitle), "", current.is_some() && !unseated, "").clicked();
                    }
                    if reg.rows.len() > 1 {
                        switch = f.go(ui, t(Key::IdSwitchTitle), &fill1(Key::CountN, &reg.rows.len().to_string()), true, "").clicked();
                    } else {
                        let _ = f.info(ui, t(Key::IdSwitchTitle), card::Show::Empty(t(Key::IdOnlyOne)));
                    }
                    new = f.go(ui, t(Key::IdNewTitle), "", true, "").clicked();
                    import = f.go(ui, t(Key::IdImportTitle), "", true, "").clicked();
                    rename = f.go(ui, t(Key::IdDoRenameGo), "", current.is_some(), "").clicked();
                });
            });
        });
        // Details: the addresses, the backup file, the derivation paths (recovery-phrase identities only), the
        // identity list file, and which domains this identity signs (from the seat × domain table).
        let registry = crate::register::path().map(|p| p.display().to_string()).unwrap_or_default();
        let seat_addr = |which: crate::roles::Role| current.as_ref().and_then(|(r, _)| r.address(which)).map(|a| Val::mono(a.hex())).unwrap_or_else(|| Val::text(t(Key::IdSeatEmpty)));
        let mut raw: Vec<(&str, Val)> = Vec::new();
        if let Some(a) = self.shell.anchor.filter(|_| !unseated) {
            raw.push((t(Key::IdAddress), Val::mono(a.hex())));
            // The public key under the key address, with its copy key: what someone sealing a record package to this
            // key fills in.
            if let Some(p) = self.shell.anchor_public {
                raw.push((t(Key::IdPublicKey), Val::mono(zikaron::hexfmt::encode(&p))));
            }
        }
        raw.push((t(Key::IdSeatRowAuthor), seat_addr(crate::roles::Role::Author)));
        raw.push((t(Key::IdSeatRowGrantee), seat_addr(crate::roles::Role::Grantee)));
        if let Some(b) = seen.as_ref().filter(|b| b.at != crate::identity::NO_BACKUP_AT) {
            raw.push((t(Key::IdBackupAt), Val::figure(b.at.clone())));
        }
        raw.push((t(Key::IdBackupAt), Val::text(backup_at.1.clone())));
        raw.push((t(Key::IdCreated), Val::figure(created.clone())));
        raw.push((t(Key::IdStore), Val::text(t(Key::IdKeybox))));
        if has_words {
            raw.push((t(Key::IdPaths), Val::mono(format!("{} \u{b7} {}", crate::family::path_text(crate::roles::Role::Author), crate::family::path_text(crate::roles::Role::Grantee)))));
        }
        raw.push((t(Key::IdRegistry), Val::mono(registry)));
        raw.push((t(Key::SignFaces), Val::mono(crate::sign::Face::ALL.iter().map(|f| f.as_str()).collect::<Vec<_>>().join(" "))));
        raw.push((
            t(Key::SignSeatDomains),
            if current.is_some() && !unseated {
                Val::mono(crate::sign::Use::all().into_iter().filter(|u| crate::sign::seat_may(seat, *u)).map(|u| u.as_str()).collect::<Vec<_>>().join(" "))
            } else {
                Val::text(t(Key::IdSeatEmpty))
            },
        ));
        let mut open = self.ux.u3.keys_details;
        stagger(ui, 4, |ui| details_card_held(ui, "keys-details", &raw, &mut open));
        self.ux.u3.keys_details = open;
        let mut delete = false;
        stagger(ui, 5, |ui| {
            if current.is_some() {
                card::form(ui, |ui, f| delete = f.danger(ui, t(Key::IdDoDeleteGo)).clicked());
            }
        });
        if copy {
            if let Some(a) = self.shell.anchor {
                ui.ctx().copy_text(a.hex());
                self.toasts.say(t(Key::SaidCopiedAddress), Tone::Note, now);
            }
        }
        if let Some(i) = lock_pick {
            let secs = crate::machine::LOCK_CHOICES[i];
            if secs != lock_now {
                self.act(Action::SetAutoLock { on: true, secs }, now);
            }
        }
        if flip {
            self.act(Action::SetAutoLock { on: !lock_on, secs: lock_now }, now);
        }
        if set_primary {
            if let Some((r, _)) = &current {
                self.id_layer_open(IdModal::Primary(r.id.clone()));
            }
        }
        if change {
            self.id_layer_open(IdModal::Pin);
        }
        if set_pin {
            self.wizard_open(crate::nav::Step::Pin);
        }
        if backup {
            self.id_layer_open(IdModal::Backup);
        }
        if words {
            self.id_layer_open(IdModal::Words);
        }
        if switch {
            self.id_layer_open(IdModal::Switch);
        }
        if new {
            self.id_layer_open(IdModal::New);
            self.act(Action::NewIdentity, now);
        }
        if import {
            self.id_layer_open(IdModal::Import);
        }
        if rename {
            if let Some((r, _)) = &current {
                let (id, label) = (r.id.clone(), r.label.clone());
                self.id_layer_open(IdModal::Name(id));
                self.ux.id_label = label;
            }
        }
        if delete {
            if let Some((r, _)) = &current {
                self.id_layer_open(IdModal::Delete(r.id.clone()));
            }
        }
    }

    fn set_network(&mut self, ui: &mut egui::Ui, now: f64) {
        let s = self.shell.settings.clone();
        let bare = s.chain_id.is_none() && s.registry.is_none() && s.endpoints.is_empty();
        // This home's network: the preset row it was filled from (a home that has a network keeps it even if an
        // identity later chooses another), or a hand-saved setup whose cells all equal a current row
        // (`deploy::same_as_row`), else custom.
        let row = s.network.as_deref().and_then(crate::deploy::named).or_else(|| crate::deploy::same_as_row(s.chain_id, s.registry, s.from_block, &s.endpoints));
        let title = match row {
            _ if bare => t(Key::SetNetworkNone).to_string(),
            Some(d) => network_label(d.name),
            None => t(Key::U3Custom).to_string(),
        };
        let (mark, said) = match self.chain_read() {
            // The last read failed: show this failure, not the reading before it.
            Err(f) => (Mark::Bad, fill1(Key::SetReadFailedNow, f.human())),
            Ok(Some(Done::Chain { sources, single_source, unanswered, .. })) => {
                let said = if *single_source { t(Key::SetSingleSource).to_string() } else { fill1(Key::SetAgreed, &sources.to_string()) };
                // Name the nodes that gave nothing, so "single source" says which one stayed silent: the head
                // counts them, the fold names them.
                let said = if unanswered.is_empty() { said } else { format!("{said} · {}", fill1(Key::SetUnansweredCount, &unanswered.len().to_string())) };
                (if *single_source || !unanswered.is_empty() { Mark::Warn } else { Mark::Ok }, said)
            }
            _ => (Mark::Todo, t(Key::SetNotRead).to_string()),
        };
        let unanswered: Vec<String> = match self.chain_read() {
            Ok(Some(Done::Chain { unanswered, .. })) => unanswered.clone(),
            _ => Vec::new(),
        };
        let tint = match (bare, mark) {
            (true, _) => card::Tint::Warn,
            (false, Mark::Ok) => card::Tint::Ok,
            (false, Mark::Warn) => card::Tint::Warn,
            (false, Mark::Bad) => card::Tint::Bad,
            _ => card::Tint::Blue,
        };
        let head = match s.chain_id {
            Some(ch) if !bare => format!("{title} \u{b7} {}", fill1(Key::ChainNumbered, &ch.to_string())),
            _ => title.clone(),
        };
        let nodes = s.endpoints.join(" ");
        let registry = s.registry.map(|a| a.hex());
        let phase = self.phase_of(crate::task::Kind::Chain);
        let (mut read_chain, mut edit, mut copied, mut flip) = (false, false, None, false);
        // The overview: the network and its chain, how the last reading went, "read the chain".
        stagger(ui, 0, |ui| {
            card::form(ui, |ui, f| {
                read_chain = f.summary(ui, Glyph::Globe, tint, &head, None, &said, |ui| key::show(ui, key::Key::new(t(Key::DoReadChain), Role::Secondary).enabled(!bare).phase(phase)).clicked());
                if !unanswered.is_empty() {
                    let _ = f.fold(ui, "chain-unanswered", t(Key::SetWhatEachSaid), &unanswered.len().to_string(), |ui, f| f.note(ui, &fill1(Key::SetUnanswered, &unanswered.join(" \u{b7} "))));
                }
            });
            self.stage_line(ui, crate::task::Kind::Chain);
        });
        // Nodes and chain: shown here, edited in their sheet; the auto put on chain switch (per home, off by
        // default) with them.
        stagger(ui, 1, |ui| {
            card::section(ui, t(Key::NetGroup), "", |ui| {
                card::form(ui, |ui, f| {
                    // The nodes in the monospace face; none yet says so in words, as the publish address does.
                    let k = key::Key::new(t(Key::SetEditNodes), if bare { Role::Primary } else { Role::Secondary });
                    edit = if nodes.is_empty() { f.act(ui, t(Key::SetNodes), t(Key::SetNone), k) } else { f.act_path(ui, t(Key::SetNodes), &nodes, k) }.clicked();
                    copied = match registry.as_deref() {
                        Some(r) => f.info(ui, t(Key::SetRegistry), card::Show::Short(r)),
                        None => f.info(ui, t(Key::SetRegistry), card::Show::Empty(t(Key::SetNone))),
                    };
                    flip = f.switch(ui, t(Key::SetAutoAnchor), "", s.auto_anchor, true);
                });
            });
        });
        tell_copied(ui.ctx(), copied);
        if flip {
            self.act(Action::SetAutoAnchor { on: !s.auto_anchor }, now);
        }
        if read_chain {
            self.act(Action::ReadChain, now);
        }
        if edit {
            self.net_edit_open();
        }
        stagger(ui, 2, |ui| self.set_read_nets(ui));
        stagger(ui, 3, |ui| self.set_publish(ui));
        stagger(ui, 4, |ui| self.set_proxy(ui, now));
        let none = || Val::text(t(Key::SetNone));
        let rows = [
            (t(Key::SetNodes), if s.endpoints.is_empty() { none() } else { Val::mono(s.endpoints.join("\n")) }),
            (t(Key::BasisChain), s.chain_id.map(|n| Val::figure(n.to_string())).unwrap_or_else(none)),
            (t(Key::SetRegistry), s.registry.map(|a| Val::mono(a.hex())).unwrap_or_else(none)),
            (t(Key::BasisFrom), Val::figure(s.from_block.to_string())),
        ];
        stagger(ui, 5, |ui| details_card(ui, "network-details", &rows));
    }

    /// Open the sheet for a new read-only network.
    pub(super) fn read_net_new_open(&mut self) {
        self.ux.readnet = ReadNetUx { sheet: Some((None, Draft::default())), armed: false };
    }

    /// Whether a sheet of the network page is open.
    pub(super) fn net_sheet_open(&self) -> bool {
        self.ux.net_sheet || self.ux.readnet.sheet.is_some() || self.ux.publish_sheet || self.ux.publish_edit
    }

    /// Close the network page's sheets.
    pub(super) fn net_sheets_clear(&mut self) {
        self.ux.net_sheet = false;
        self.ux.basis_waiting = false;
        self.ux.readnet = ReadNetUx::default();
        self.ux.publish_sheet = false;
        self.ux.publish_edit = false;
    }

    /// Open the network sheet with this home's network as saved.
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

    /// The cells of the network sheet as one of the known table's rows fills them.
    fn net_fill(&mut self, d: &crate::deploy::Deployment) {
        let c = d.cells();
        self.ux.basis_chain = c.chain;
        self.ux.basis_registry = c.registry;
        self.ux.basis_from = c.from;
        self.typed.endpoints = c.endpoints;
    }

    /// The network sheet's cells that do not read yet, by name.
    fn net_missing(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.typed.endpoints.trim().is_empty() {
            v.push(t(Key::SetNodes));
        }
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

    /// The known table's row the network sheet's cells equal now, if any.
    fn net_row_typed(&self) -> Option<usize> {
        crate::deploy::KNOWN.iter().position(|d| {
            let c = d.cells();
            c.chain == self.ux.basis_chain.trim() && c.registry.eq_ignore_ascii_case(self.ux.basis_registry.trim()) && c.from == self.ux.basis_from.trim() && c.endpoints.split_whitespace().eq(self.typed.endpoints.split_whitespace())
        })
    }

    /// The network sheet: Ethereum mainnet or custom on its first line (custom offers the known table's rows
    /// as presets beside it), the four cells, one key to save. Saving writes the chain settings (a registry
    /// outside the known table is checked at this home's nodes first, in the background) and then the nodes;
    /// the sheet closes once both are kept, and stays with the reading when the check says otherwise.
    pub(super) fn net_edit_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if !self.ux.net_sheet {
            return;
        }
        let checking = self.shell.tasks.in_flight(crate::task::Kind::Basis);
        let typed = (self.ux.basis_chain.trim().parse::<u64>().ok(), crate::key::Address::parse(&self.ux.basis_registry));
        // The check this sheet started has landed: a reading that the registry is the pinned build closes it.
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
            // No reading yet (no node for this chain, or the check still running): never shown as checked.
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
                let row = me.net_row_typed();
                let custom = me.ux.basis_custom || row != Some(0);
                let cells = [seg::Cell::from(t(crate::deploy::KNOWN[0].label)), seg::Cell::from(t(Key::U3Custom))];
                let (mut pick, mut preset) = (None, None);
                fields::stack(ui, |ui, st| {
                    // Mainnet or custom; the preset menu beside it on the right while custom (its place kept).
                    ui.horizontal(|ui| {
                        pick = seg::seg(ui, "network-kind", &cells, usize::from(custom));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let b = if custom { egui::UiBuilder::new() } else { egui::UiBuilder::new().invisible() };
                            ui.scope_builder(b, |ui| preset = preset_menu(ui, "network-preset", row));
                        });
                    });
                    let mono = fields::Look { mono: true, ..Default::default() };
                    let check = reading.map(|(m, k)| (t(k), m == Mark::Bad));
                    st.field(ui, t(Key::SetNodes), None, None, |ui, w| fields::line(ui, &mut me.typed.endpoints, t(Key::EndpointHint), w, mono));
                    st.pair(
                        ui,
                        |ui, st| st.field(ui, t(Key::BasisChain), None, None, |ui, w| fields::line(ui, &mut me.ux.basis_chain, "", w, mono)),
                        |ui, st| st.field(ui, t(Key::BasisFrom), None, None, |ui, w| fields::line(ui, &mut me.ux.basis_from, "", w, mono)),
                    );
                    st.field(ui, t(Key::SetRegistry), None, check, |ui, w| fields::line(ui, &mut me.ux.basis_registry, "", w, mono));
                });
                match pick {
                    Some(0) => {
                        me.ux.basis_custom = false;
                        me.net_fill(&crate::deploy::KNOWN[0]);
                    }
                    // Custom: the cells start empty, the preset menu shows.
                    Some(_) if !custom => {
                        me.ux.basis_custom = true;
                        me.ux.basis_chain.clear();
                        me.ux.basis_registry.clear();
                        me.ux.basis_from.clear();
                        me.typed.endpoints.clear();
                    }
                    _ => {}
                }
                if let Some(i) = preset {
                    me.net_fill(&crate::deploy::KNOWN[i]);
                }
            },
            |ui, me| {
                // Saving waits until every cell reads; what is still missing is said on the key, on hover.
                let missing = me.net_missing();
                let k = key::Key::new(t(Key::ReadNetSave), Role::Primary).phase(phase).enabled(missing.is_empty());
                let still = fill1(Key::GrantStillNeed, &missing.join(t(Key::ListJoin)));
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
        // The chain settings first (their check asks this home's nodes for that chain), then the nodes (saving
        // them checks a custom network again on the new nodes).
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

    /// The proxy (machine-wide), one card: follow the system, none, or a custom proxy in cells (kind, host,
    /// port, user name, password; checked in `proxycells`), saved at the card's foot. A wrong cell is framed red
    /// with its reason under it, and saving waits until every cell is valid. A whole address pasted into the
    /// host cell, or left there, is split into the cells. A saved password is never shown again: its cell stays
    /// empty, and the line under it says one is set only while saving the cells as they are would keep it
    /// (`proxycells::password_stays`: the same proxy and user name). The cells come and go in place.
    fn set_proxy(&mut self, ui: &mut egui::Ui, now: f64) {
        use crate::machine::proxy::{NONE, SYSTEM};
        use crate::proxycells::{self as pc, Bad};
        let written = self.shell.machine.proxy.clone();
        let cur = match written.as_deref().map(str::trim) {
            None | Some(SYSTEM) => 0,
            Some(NONE) => 1,
            Some(_) => 2,
        };
        let saved = written.as_deref().filter(|_| cur == 2).and_then(pc::of_saved);
        if cur == 2 && !self.ux.proxy_seeded {
            if let Some((cells, _)) = &saved {
                self.typed.proxy = cells.clone();
            }
            self.ux.proxy_seeded = true;
        }
        let shown = if self.ux.proxy_manual { 2 } else { cur };
        let choices: Vec<seg::Cell> = [Key::ProxySystem, Key::ProxyOff, Key::ProxyManual].iter().map(|k| seg::Cell::from(t(*k))).collect();
        let open = self.ux.proxy_manual || cur == 2;
        let c = &mut self.typed.proxy;
        let checked = pc::check(c);
        let is_bad = |b: Bad| matches!(&checked, Err(v) if v.contains(&b));
        let (mut pick, mut hit) = (None, false);
        let mut to_save = None;
        card::form(ui, |ui, f| {
            f.keep(ui);
            f.key("proxy");
            pick = f.with(ui, t(Key::SetProxy), "", |ui| seg::seg(ui, "set-proxy", &choices, shown));
            if !open {
                return;
            }
            f.key("cells");
            f.panel(ui, |ui| {
                fields::stack(ui, |ui, st| {
                    let kinds: Vec<seg::Cell> = pc::KINDS.iter().map(|(_, w)| seg::Cell::from(*w)).collect();
                    let at = pc::KINDS.iter().position(|(k, _)| *k == c.kind).unwrap_or(0);
                    if let Some(i) = st.field(ui, t(Key::U3RawType), None, None, |ui, _| seg::seg(ui, "proxy-kind", &kinds, at)) {
                        c.kind = pc::KINDS[i].0;
                    }
                    let host_bad = is_bad(Bad::Host) && !c.host.trim().is_empty();
                    let port_bad = is_bad(Bad::Port) && !c.port.trim().is_empty();
                    let (host, _) = st.pair_w(
                        ui,
                        128.0,
                        |ui, st| {
                            st.field(ui, t(Key::ProxyCellHost), None, None, |ui, w| {
                                fields::line(ui, &mut c.host, t(Key::ProxyHint), w, fields::Look { mono: true, bad: host_bad, ..Default::default() })
                            })
                        },
                        |ui, st| {
                            st.field(ui, t(Key::ProxyCellPort), None, None, |ui, w| {
                                fields::line(ui, &mut c.port, "", w, fields::Look { mono: true, bad: port_bad, ..Default::default() })
                            })
                        },
                    );
                    // A whole address pasted at once, or left in the host cell, is split into the cells
                    // (`proxycells::split_in`): never mid-way while it is typed.
                    let pasted: Option<String> = ui.input(|i| {
                        let p: String = i.events.iter().filter_map(|e| if let egui::Event::Paste(p) = e { Some(p.as_str()) } else { None }).collect();
                        (!p.is_empty()).then_some(p)
                    });
                    if let Some(split) = pc::split_in(c, pasted.as_deref(), host.has_focus()) {
                        *c = split;
                    }
                    let user_bad = (is_bad(Bad::UserMissing) || is_bad(Bad::UserLong)) && !(c.user.is_empty() && c.pass.is_empty());
                    let pass_bad = is_bad(Bad::PassLong);
                    let user_label = if pc::takes_password(c.kind) { Key::ProxyCellUser } else { Key::ProxyCellUserId };
                    let takes = pc::takes_password(c.kind);
                    st.pair(
                        ui,
                        |ui, st| {
                            st.field(ui, t(user_label), Some(t(Key::ProxyCellOptional)), None, |ui, w| {
                                fields::line(ui, &mut c.user, "", w, fields::Look { bad: user_bad, ..Default::default() })
                            })
                        },
                        |ui, st| {
                            if takes {
                                st.field(ui, t(Key::IdPassword), Some(t(Key::ProxyCellOptional)), None, |ui, w| input::secret_line_w_marked(ui, &mut c.pass, "", w, pass_bad));
                            }
                        },
                    );
                    // Why a cell is red, one line each; else whether a password is kept ("password set" only when
                    // saving these cells now would keep the saved password: the same check saving uses).
                    let mut lines: Vec<(Mark, &str)> = Vec::new();
                    if let Err(v) = &checked {
                        for b in v {
                            let shown = match b {
                                Bad::Host => host_bad,
                                Bad::Port => port_bad,
                                Bad::UserMissing | Bad::UserLong => user_bad,
                                Bad::PassLong => true,
                            };
                            if shown {
                                lines.push((
                                    Mark::Bad,
                                    t(match b {
                                        Bad::Host => Key::ProxyBadHost,
                                        Bad::Port => Key::ProxyBadPort,
                                        Bad::UserMissing => Key::ProxyBadUserMissing,
                                        Bad::UserLong => Key::ProxyBadUserLong,
                                        Bad::PassLong => Key::ProxyBadPassLong,
                                    }),
                                ));
                            }
                        }
                    }
                    let saved_proxy = written.as_deref().and_then(|w| zikaron_net::proxy_of(w).ok());
                    let typed_proxy = pc::check(c).ok().and_then(|a| zikaron_net::proxy_of(&a).ok());
                    if takes && matches!((&saved_proxy, &typed_proxy), (Some(s), Some(t_)) if pc::password_stays(s, t_)) {
                        lines.push((Mark::Ok, t(Key::ProxyPasswordSet)));
                    }
                    st.says(ui, &lines);
                    // Saving: every cell reads, and the cells differ from what is saved (a password typed counts).
                    let typed_pass = !c.pass.is_empty() && takes;
                    let shown_written = written.as_deref().map(|w| zikaron_net::proxy_text_shown(w).0).unwrap_or_default();
                    to_save = pc::check(c).ok().filter(|a| cur != 2 || typed_pass || zikaron_net::proxy_text_shown(a).0 != shown_written);
                    st.foot(ui, |ui| hit = key::key(ui, t(Key::ReadNetSave), Role::Secondary, to_save.is_some()).clicked());
                });
            });
        });
        match pick {
            Some(2) => self.ux.proxy_manual = true,
            // The saved choice pressed again (from "enter one" left unsaved): back to it, nothing written.
            Some(i) if i == cur => self.ux.proxy_manual = false,
            Some(i) if i != shown => {
                self.ux.proxy_manual = false;
                self.act(Action::SetProxy { choice: if i == 0 { SYSTEM } else { NONE }.to_string() }, now);
            }
            _ => {}
        }
        if let (true, Some(address)) = (hit, to_save) {
            self.act(Action::SetProxy { choice: address }, now);
            if self.shell.machine.proxy.as_deref().is_some_and(|p| p != SYSTEM && p != NONE) {
                self.ux.proxy_manual = false;
                self.ux.proxy_seeded = false;
                self.typed.proxy.pass.clear();
            }
        }
    }

    /// The read-only networks (machine-wide): a list, one row each with its name and reading; a row opens its
    /// sheet, and "Add network", the last row, opens the same sheet empty.
    fn set_read_nets(&mut self, ui: &mut egui::Ui) {
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

    /// A read-only network's sheet: its name (a new one's name cell offers the known table's rows as presets),
    /// chain id, registry, start block and nodes; its reading with "read" (a saved one); "remove" at the bottom
    /// left, asked once more in the sheet; saving closes it.
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
                // A saved network's reading, with "read the chain".
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
                let still = fill1(Key::GrantStillNeed, &missing.join(t(Key::ListJoin)));
                if key::show(ui, if missing.is_empty() { k } else { k.why(&still) }).clicked() {
                    act = Some(d.save(was));
                }
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                // Delete at the bottom left: pressed once it asks ("confirm delete"), pressed again it deletes.
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

    /// From the verify page, for a kit naming a network not yet added: open the settings' network page with the
    /// network sheet holding the kit's chain, registry and start block (replacing any unsaved one); the user
    /// fills in the nodes.
    pub(super) fn add_stated_network(&mut self, at: &crate::kitsindex::AnchoredOn, now: f64) {
        let d = Draft { preset: None, name: String::new(), chain: at.chain_id.to_string(), registry: at.registry.clone(), from: at.from_block.to_string(), nodes: String::new() };
        self.go(Place::Settings(Section::Network), now);
        self.ux.readnet = ReadNetUx { sheet: Some((None, d)), armed: false };
    }

    /// The publish address (https only, checked at once) in its field, saved beside it; "check publication"
    /// opens its sheet.
    fn set_publish(&mut self, ui: &mut egui::Ui) {
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

    /// The publish address's sheet: the address (https only, checked as it is typed), "save".
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
                save = key::key(ui, t(Key::DoSetPublish), Role::Primary, differs && shape.is_none()).clicked();
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

    /// "Check publication": the record kit to compare (the latest kit by default, the last one chosen after),
    /// "choose folder" at the bottom left, and what the check found, one sentence for each of three outcomes.
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
                    states::err_box(ui, "publish-err", t(Key::PublishUnreachable), f.human(), t(Key::U3RawError), &f.raw());
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
                go = key::show(ui, key::Key::new(t(Key::DoCheckShort), Role::Primary).enabled(!me.typed.publish_local.trim().is_empty()).phase(phase)).clicked();
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

    fn set_notify(&mut self, ui: &mut egui::Ui, now: f64) {
        let s = self.shell.settings.clone();
        // Both intervals in their rows (each runs in either seat): a narrow cell (empty means by hand only), its
        // unit, "save" beside it. A cell shows the saved interval again whenever that changes.
        if self.ux.cadence_seed != Some((s.audit_every, s.review_every)) {
            self.ux.audit_every = if s.audit_every == 0 { String::new() } else { s.audit_every.to_string() };
            self.ux.review_every = if s.review_every == 0 { String::new() } else { s.review_every.to_string() };
            self.ux.cadence_seed = Some((s.audit_every, s.review_every));
        }
        let read = |x: &str| if x.trim().is_empty() { Some(0) } else { x.trim().parse::<u64>().ok() };
        let (audit, review) = (read(&self.ux.audit_every), read(&self.ux.review_every));
        let (mut save_audit, mut save_review) = (false, false);
        // The two save keys are as wide as the wider of them, so the fields before them line up.
        let key_w = [Key::DoSetAuditEvery, Key::DoSetReviewEvery].iter().map(|k| key::size_of(ui, &key::Key::new(t(*k), Role::Secondary)).x).fold(92.0_f32, f32::max);
        let ux = &mut self.ux;
        stagger(ui, 0, |ui| {
            card::form(ui, |ui, f| {
                f.with(ui, t(Key::PageAudit), "", |ui| {
                    save_audit = key::show_w(ui, key::Key::new(t(Key::DoSetAuditEvery), Role::Secondary).enabled(audit.is_some_and(|n| n != s.audit_every)).why(if audit.is_none() { t(Key::U3EveryMustBeNumber) } else { "" }), key_w).clicked();
                    ui.label(egui::RichText::new(t(Key::UnitSeconds)).font(zikaron_ui::tokens::Type::Note.font()).color(c(C::Ink2)));
                    let _ = fields::small(ui, &mut ux.audit_every, t(Key::SetEveryOff), 110.0, audit.is_none());
                });
                f.with(ui, t(Key::SetReview), "", |ui| {
                    save_review = key::show_w(ui, key::Key::new(t(Key::DoSetReviewEvery), Role::Secondary).enabled(review.is_some_and(|n| n != s.review_every)).why(if review.is_none() { t(Key::U3EveryMustBeNumber) } else { "" }), key_w).clicked();
                    ui.label(egui::RichText::new(t(Key::UnitSeconds)).font(zikaron_ui::tokens::Type::Note.font()).color(c(C::Ink2)));
                    let _ = fields::small(ui, &mut ux.review_every, t(Key::SetEveryOff), 110.0, review.is_none());
                });
            });
        });
        if let (true, Some(n)) = (save_audit, audit) {
            self.act(Action::SetAuditEvery { secs: n }, now);
        }
        if let (true, Some(n)) = (save_review, review) {
            self.act(Action::SetReviewEvery { secs: n.to_string() }, now);
        }
    }

    /// A restored identity whose ledger is not fetched yet: what is wrong, where to fetch from, and "fetch
    /// ledger". Nothing is shown without the mark.
    fn set_fetch(&mut self, ui: &mut egui::Ui, now: f64) {
        let Some(s) = self.shell.unfetched else { return };
        let title = match s {
            crate::restorex::State::Unfetched => t(Key::FetchLedgerTitle).to_string(),
            crate::restorex::State::NewerElsewhere { missing } => fill1(Key::FetchNewerTitle, &missing.to_string()),
        };
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Fetch);
        let (mut pick, mut go) = (false, false);
        card::card(ui, |ui| {
            ui.spacing_mut().item_spacing.y = tk::S3;
            states::banner(ui, states::Banner::Bad, &title, |_| ());
            // The ledger comes back from a whole-machine backup (this identity's ledger for this seat).
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

    /// The old data on this machine (homes kept after a conflict): when, how many entries and how many never
    /// anchored; "view" opens one to read.
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
        // What is here, how it is kept, how much room it takes (every number from the last background
        // measurement; the frame reads no disk). Measured again once the ledger moved, whoever wrote.
        if self.shell.take_measure_due() {
            self.auto(Action::Measure, now);
        }
        let ids = self.shell.identities.as_ref().map(|r| r.rows.len()).unwrap_or(0);
        let archive = self.shell.archive.clone();
        let content = match archive.as_ref() {
            Some(a) => crate::lang::filln(Key::BackupContent, &[&ids.to_string(), &a.items.to_string(), &a.records.to_string()]),
            None => t(Key::SetNotRead).to_string(),
        };
        let strays = archive.as_ref().filter(|a| a.skipped > 0).map(|a| fill2(Key::SetStraysSay, &a.items.to_string(), &a.skipped.to_string()));
        let usage = archive.as_ref().map(|a| fill2(Key::SetUsageHuman, &size_say(a.bytes), &size_say(self.shell.settings.cap_bytes))).unwrap_or_else(|| t(Key::SetNotRead).to_string());
        let over = archive.as_ref().is_some_and(|a| a.bytes > self.shell.settings.cap_bytes);
        // On an empty seat, keys that need a home or a key are disabled, as on the identity page
        // (`Shell::seat_unseated`).
        let seat_ok = !self.shell.seat_unseated();
        let held = self.shell.pen != Pen::Granted;
        let pen = if held { t(Key::SetPenHeld) } else { t(Key::SetPenOpen) };
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
        // The overview: how the data is kept and whether it writes, "reconcile now"; three figures: the
        // entries, the last reconciliation, the last backup.
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
        // The whole-machine backup: export, restore, and (the recorder's) the ledger mirror, rows of one kind.
        let author = self.shell.settings.role == crate::roles::Role::Author;
        let (mut export, mut restore, mut mirror) = (false, false, false);
        stagger(ui, 1, |ui| {
            card::section(ui, t(Key::WizBackupTitle), "", |ui| {
                card::form(ui, |ui, f| {
                    export = f.go(ui, t(Key::BackupExportTitle), "", self.shell.unlocked(), "").clicked();
                    restore = f.go(ui, t(Key::BackupRestoreTitle), "", true, "").clicked();
                    if author {
                        mirror = f.go(ui, t(Key::MirrorTitle), "", true, "").clicked();
                    }
                });
            });
        });
        if export {
            self.bk_open(Bk::Export);
        }
        if restore {
            self.bk_open(Bk::Restore(RestoreFrom::Settings));
        }
        if mirror {
            self.bk_open(Bk::Mirror);
        }
        // The data folder (its path under the name, elided in the middle, whole on hover) with "change"; the usage
        // with "measure"; for the recorder, the switch that hides what a deletion leaves on this machine only.
        let root = self.shell.home.as_ref().map(|h| h.root().display().to_string());
        let measure_phase = self.phase_of(crate::task::Kind::Archive);
        let (mut open, mut measure, mut flip) = (None, false, false);
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
            });
            self.stage_line(ui, crate::task::Kind::Archive);
            if over {
                states::okline(ui, Mark::Bad, &usage);
            }
        });
        // Advanced: the usage limit in its row; move, adopt, import grants, check every anchor again: one key
        // each, a sheet for those that take a folder.
        let grantee = !author;
        let cap = self.shell.settings.cap_bytes;
        if self.ux.cap_seed != Some(cap) {
            self.typed.cap = cap.to_string();
            self.ux.cap_seed = Some(cap);
        }
        let typed_cap = self.typed.cap.trim().parse::<u64>().ok();
        let migrating = self.phase_of(crate::task::Kind::Migrate);
        let (mut set_cap, mut recheck) = (false, false);
        let typed = &mut self.typed;
        stagger(ui, 3, |ui| {
            card::section(ui, t(Key::U3MoreOptions), "", |ui| {
                card::form(ui, |ui, f| {
                    f.with(ui, t(Key::SetCap), "", |ui| {
                        set_cap = key::show_w(ui, key::Key::new(t(Key::DoSetCap), Role::Secondary).enabled(seat_ok && typed_cap.is_some_and(|n| n != cap)).why(if typed_cap.is_none() { t(Key::CapMustBeNumber) } else { "" }), 92.0).clicked();
                        ui.label(egui::RichText::new(t(Key::UnitBytes)).font(zikaron_ui::tokens::Type::Note.font()).color(c(C::Ink2)));
                        let _ = fields::small(ui, &mut typed.cap, "", 130.0, typed_cap.is_none());
                    });
                    if f.act(ui, t(Key::MigrateLabel), "", key::Key::new(t(Key::DoMigrate), Role::Secondary).enabled(seat_ok).phase(migrating).busy_text(t(Key::SetMigrating))).clicked() {
                        open = Some(DirSheet::Migrate);
                    }
                    if f.act(ui, t(Key::AdoptGroup), "", key::Key::new(t(Key::DoAdopt), Role::Secondary).enabled(seat_ok)).clicked() {
                        open = Some(DirSheet::Adopt);
                    }
                    if grantee && f.go(ui, t(Key::V2ImportGrantDir), "", seat_ok, "").clicked() {
                        open = Some(DirSheet::GrantDir);
                    }
                    // Anchors that several nodes already confirmed alike are not queried again (`checkedx`); this
                    // drops that record, so the next sync queries every anchor.
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
        if let (true, Some(n)) = (set_cap, typed_cap) {
            self.act(Action::SetCap { bytes: n }, now);
        }
        if recheck {
            self.act(Action::RecheckAll, now);
        }
    }

    /// Open a sheet that takes a folder: the folder last chosen for it, else the documents folder.
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

    /// The typed folder of a sheet.
    fn dir_slot(&mut self, d: DirSheet) -> &mut String {
        match d {
            DirSheet::Home => &mut self.typed.home,
            DirSheet::GrantDir => &mut self.typed.grant_dir,
            DirSheet::Migrate => &mut self.typed.migrate,
            DirSheet::Adopt => &mut self.typed.adopt,
        }
    }

    /// Where a move would land for the folder chosen: worked out once, when the folder is chosen (it walks
    /// the disk); the frame only reads it.
    fn dir_landing(&mut self) {
        let at = self.typed.migrate.trim().to_string();
        self.ux.migrate_landing = (!at.is_empty()).then(|| crate::home::choose(&crate::home::Kind::Home, std::path::Path::new(&at)));
    }

    /// A sheet that takes a folder: the folder in one row, "choose folder" at the bottom left, cancel
    /// and the action at the bottom right. Moving the data keeps the sheet up until the copy is done.
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
                let entries = me.shell.archive.as_ref().map(|a| a.items.to_string());
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
                // Where the move lands, when not in the folder chosen itself.
                if let Some(k) = landing.as_ref().and_then(|p| p.why.say_ahead().map(|k| (k, p))) {
                    hint(ui, &fill1(k.0, &width::file_name(&k.1.at.display().to_string())));
                }
            },
            |ui, _me| {
                // Moving and adopting commit the data (the pen's key); the others open or import.
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
                // Another home may have chosen another language: decide again next frame.
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
                    // A chosen place is used once: after the move it is no longer empty.
                    self.typed.migrate.clear();
                    self.ux.migrate_landing = None;
                }
            }
        }
    }

    /// How many ledger entries and held grants came after the last whole-machine backup (from the last
    /// measurement and the machine settings; `None` when never backed up or not measured).
    pub(super) fn backup_behind(&self) -> Option<u64> {
        crate::machine::backup_behind(self.shell.machine.backup.as_ref(), self.shell.items_now)
    }

    /// The backup point's mark and sentence (the setup check and the wizard use the same judgment,
    /// `firstrun::backup_point`).
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
        let title = format!("{} {}", t(Key::AppName), env!("CARGO_PKG_VERSION"));
        // The overview: the app and its version, the build under it.
        stagger(ui, 0, |ui| {
            card::form(ui, |ui, f| f.summary(ui, Glyph::Info, card::Tint::Blue, &title, None, build_kind(), |_| ()));
        });
        // The setup check: a point done says so with its mark; one still to set up has the key that sets it up.
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
        let mut go = None;
        let (mut again, mut self_check) = (false, false);
        stagger(ui, 1, |ui| {
            card::section(ui, t(Key::SetMissing), "", |ui| {
                card::form(ui, |ui, f| {
                    for ((m, name, _), p) in points.iter().zip(&goes) {
                        if *m == Mark::Ok {
                            // A done point says how it stands when there is a word for it (ready, done).
                            let word = match p {
                                Point::Step(crate::nav::Step::Genesis) | Point::Backup => t(Key::Done),
                                Point::Step(crate::nav::Step::Gas) | Point::Network => t(Key::PointReady),
                                _ => "",
                            };
                            let _ = f.info(ui, name, card::Show::Mark(Mark::Ok, word));
                            continue;
                        }
                        let label = match p {
                            Point::Step(s) => t(s.title()),
                            Point::Backup => t(Key::BackupExportTitle),
                            Point::Network => t(Key::SetEditNodes),
                        };
                        if f.act(ui, name, "", key::Key::new(label, Role::Secondary).enabled(!unseated)).clicked() {
                            go = Some(*p);
                        }
                    }
                });
            });
        });
        stagger(ui, 2, |ui| {
            card::form(ui, |ui, f| {
                // Disabled on an empty seat, as on the identity page.
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
            None => {}
        }
        // Each font face: file, index, origin (the two embedded ones with their licence).
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
        // The third-party licences embedded in the binary (generated from Cargo.lock at build time): the crates,
        // then each licence text; only the lines in view are laid out.
        stagger(ui, 3, |ui| {
            card::form(ui, |ui, f| {
                let _ = f.fold(ui, "about-notices", t(Key::SetNotices), &crate::about::count().to_string(), |ui, f| {
                    f.free(ui, |ui| {
                        let lines = crate::about::notice_lines();
                        // One row height for drawing each line (its line height set to it) and for the scroll
                        // area's row placement, with no item spacing: rows counted and rows drawn share one pitch,
                        // so the view fills to the bottom.
                        let row_h = Type::MonoSmall.line();
                        ui.spacing_mut().item_spacing.y = 0.0;
                        // Scroll both ways so a licence line is never cut short.
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

    /// The setup check for this seat: each point's mark, name and what to do (real state only).
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
                    // Genesis counts only with a root (read when the home opened), never by a file count.
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

    /// The balance point's mark and sentence. No node means no reading, not zero.
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

/// Where a setup point still to set up is set up.
#[derive(Clone, Copy, Debug)]
enum Point {
    Step(crate::nav::Step),
    Backup,
    Network,
}

/// The sheets of the data page that take a folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DirSheet {
    /// Change the data folder.
    Home,
    /// Import a folder of grants.
    GrantDir,
    /// Move the data to an empty folder.
    Migrate,
    /// Adopt an existing ledger folder in place.
    Adopt,
}

/// What the settings page keeps for the read-only networks while one is edited (interface only).
#[derive(Default)]
pub(super) struct ReadNetUx {
    /// The open sheet: the network it edits (none for a new one), and what is typed into it.
    sheet: Option<(Option<(u64, crate::key::Address)>, Draft)>,
    /// "Remove" asks its question.
    armed: bool,
}

/// The cells of one row as typed.
#[derive(Clone, Default)]
pub(super) struct Draft {
    /// Which row of the known table filled it.
    preset: Option<usize>,
    name: String,
    chain: String,
    registry: String,
    from: String,
    nodes: String,
}

/// The preset menu of a network sheet (the main network's and a new read-only network's): the rows of the known
/// deployments table (`deploy::KNOWN`), in order; the row the cells equal now is checked. Returns the row picked.
pub(super) fn preset_menu(ui: &mut egui::Ui, salt: &str, at: Option<usize>) -> Option<usize> {
    let labels: Vec<&str> = crate::deploy::KNOWN.iter().map(|d| t(d.label)).collect();
    let items: Vec<menu::Item> = labels.iter().enumerate().map(|(i, l)| menu::Item::Row(menu::Row { label: l, check: Some(at == Some(i)), ..Default::default() })).collect();
    menu::menu_key(ui, salt, at.map(|i| labels[i]).unwrap_or(t(Key::ReadNetPreset)), false, 160.0, &items)
}

/// The same menu as a flat key inside a joined input (a read-only network's name).
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

    /// The cells that do not read yet, by name.
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

/// A reading's tone.
fn read_tone(r: crate::widex::Reading) -> PillTone {
    use crate::widex::Reading as R;
    match r {
        R::Agreed(_) => PillTone::Ok,
        R::Single => PillTone::Warn,
        R::Down | R::Fingerprint => PillTone::Bad,
    }
}

/// The cells of a read-only network's sheet: the name (fixed for a chain the name table knows) with the known
/// table's rows as presets inside its field, the nodes one per line, the chain id beside the start block, the
/// registry.
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
        st.field(ui, t(Key::SetRegistry), None, None, |ui, w| fields::line(ui, &mut d.registry, "", w, mono));
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

/// The validity's three cells on the "write alone" sheet, in order: the unit's word, the most the cell takes,
/// and the unit in seconds. Together they run from a minute to a year (`machine::LASTS_MIN`, `LASTS_MAX`).
const CLI_SPAN: [(Key, u64, u64); 3] = [(Key::CliLastsDays, 365, 86_400), (Key::CliLastsHours, 23, 3_600), (Key::CliLastsMinutes, 59, 60)];
/// The number in each third of the validity's box, and the room between it and its unit (`zikaron_ui::lasts`).
pub(super) const CLI_SPAN_NUM_W: f32 = zikaron_ui::lasts::NUM_W;
pub(super) const CLI_SPAN_GAP: f32 = zikaron_ui::lasts::GAP;

/// The validity as typed on the "write alone" sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Span {
    /// Nothing typed yet.
    Empty,
    /// These cells (days, hours, minutes) are past their most.
    Over([bool; 3]),
    /// Every cell zero.
    Zero,
    /// Longer than a year.
    PastYear,
    /// A validity that may be chosen (`machine::Lasts::ok`).
    Lasts(crate::machine::Lasts),
}

/// The validity as typed: the three cells (an empty cell is zero), or never when the switch is on (the cells are
/// then not read).
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

/// The line under the switch: when a validity expires from `now`, or why it cannot be chosen (and whether that
/// is a refusal, said in red); empty while nothing is typed or it never expires.
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

/// When the command line's credential expires, said for people (`None`: never).
pub(super) fn cli_expiry_said(expires: Option<u64>) -> String {
    match expires {
        Some(at) => fill1(Key::CliAloneUntil, &crate::when::when(at)),
        None => t(Key::CliLastsNever).to_string(),
    }
}

#[cfg(test)]
mod cli_lasts_tests {
    use super::*;

    /// The line under the validity box takes no room while it has no words; words open it to its height (a gap and
    /// a small line), passing through the heights between, and it closes again when they go.
    #[test]
    fn the_line_under_the_box_takes_room_only_with_words() {
        let ctx = egui::Context::default();
        let _ = zikaron_ui::skin::dress(&ctx);
        let mut t = 1.0;
        let mut run = |words: &str, dt: f64| {
            t += dt;
            let mut h = 0.0;
            let input = egui::RawInput { time: Some(t), ..Default::default() };
            let _ = ctx.run_ui(input, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let top = ui.cursor().min.y;
                passing_line(ui, egui::Id::new("line"), words, false);
                h = ui.cursor().min.y - top;
            });
            h
        };
        assert_eq!(run("", 0.1), 0.0, "no words, no room");
        let full = tk::S2 + Type::Small.line();
        let mut seen = vec![run("Until 2026-10-21", 0.016)];
        for _ in 0..30 {
            seen.push(run("Until 2026-10-21", 0.016));
        }
        assert!((seen.last().copied().unwrap_or(0.0) - full).abs() < 0.5, "open: {seen:?}");
        assert!(seen.iter().any(|h| *h > 0.5 && *h < full - 0.5), "through the heights between: {seen:?}");
        let mut gone = 1.0;
        for _ in 0..30 {
            gone = run("", 0.016);
        }
        assert_eq!(gone, 0.0, "closed again once the words go");
    }

    fn cells(d: &str, h: &str, m: &str) -> [String; 3] {
        [d.to_string(), h.to_string(), m.to_string()]
    }

    /// Days, hours and minutes add up (an empty cell is zero) to a validity from a minute to a year; a cell past
    /// its most, all zero or past a year is no validity; never, with the switch on, whatever the cells hold.
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

    /// A cell keeps digits only, without leading zeros; a typed zero stays one zero.
    #[test]
    fn a_cell_keeps_its_digits() {
        assert_eq!(zikaron_ui::lasts::digits("0012"), "12");
        assert_eq!(zikaron_ui::lasts::digits("1a2"), "12");
        assert_eq!(zikaron_ui::lasts::digits("000"), "0");
        assert_eq!(zikaron_ui::lasts::digits("x"), "");
    }

    /// The line under the switch: the expiry from now for a validity, each cell past its most by name, zero and
    /// past a year in red; nothing while empty or never.
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

/// A line under a field that takes room only while it has words (`bad`: in the warning ink): it opens to its height
/// and fades in as words come, and closes and fades out with the last words as they go, the sheet following it.
fn passing_line(ui: &mut egui::Ui, id: egui::Id, words: &str, bad: bool) {
    let open = motion::flag(ui.ctx(), id.with("open"), !words.is_empty(), tk::MID);
    let last = id.with("last");
    if !words.is_empty() {
        ui.ctx().data_mut(|d| d.insert_temp(last, (words.to_string(), bad)));
    }
    if open <= 0.0 {
        return;
    }
    let (said, bad) = if words.is_empty() { ui.ctx().data(|d| d.get_temp::<(String, bool)>(last)).unwrap_or_default() } else { (words.to_string(), bad) };
    if open < 1.0 {
        zikaron_ui::flow::mark_moving(ui.ctx());
    }
    let full = tk::S2 + Type::Small.line();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), full * open), egui::Sense::hover());
    let line = egui::Rect::from_min_size(egui::pos2(rect.left(), rect.top() + tk::S2), egui::vec2(rect.width(), Type::Small.line()));
    let ink = if bad { c(C::BadInk) } else { c(C::Ink2) };
    paint::at(&ui.painter().with_clip_rect(rect.intersect(ui.clip_rect())), ui, line.left_center(), egui::Align2::LEFT_CENTER, &said, Type::Small, ink.gamma_multiply(open), line.width());
}
