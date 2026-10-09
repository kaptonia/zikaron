//! Settings: the home (one line per section: title, what it holds, a chevron) and the eight section pages.
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
                    if f.nav_big(ui, t(s.key()), t(s.note(role))).clicked() {
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
        stagger(ui, 0, |ui| {
            card::form(ui, |ui, f| {
                f.row_with(ui, None, t(Key::Language), "", card::Value::None, |ui| pick_lang = seg::seg(ui, "set-lang", &lang_cells, cur));
            });
        });
        stagger(ui, 1, |ui| {
            card::form(ui, |ui, f| {
                f.row_with(ui, None, t(Key::SetZoneTitle), &example, card::Value::None, |ui| pick_zone = seg::seg(ui, "set-zone", &zone_cells, zone_now));
            });
            if crate::when::zone() == crate::when::Zone::System && !crate::when::system_read() {
                hint(ui, t(Key::ZoneSystemUnread));
            }
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

    /// "Enable command line": a switch the size of the one above, with a line under it only when there is
    /// something to say (installed, taken by another install, provided by a package, or not supported here, with
    /// the reason folded). Its state is reread whenever the row is shown again after a pause (by the action
    /// layer, never the frame); when it cannot be installed, the switch does not flip.
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
        let mut flip = false;
        stagger(ui, 3, |ui| {
            card::form(ui, |ui, f| {
                f.row_with(ui, None, t(Key::SetCliPath), &say, card::Value::None, |ui| flip = toggle::switch(ui, on, can && !busy).clicked());
            });
            // Why it cannot be installed: the details, folded.
            if let State::Unsupported(why) = &state {
                fold::fold(ui, "cli-path-why", t(Key::SetEvidence), |ui| hint(ui, why));
            }
        });
        if flip {
            self.act(Action::SetCliPath { on: !on }, now);
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
        let mut copy = false;
        stagger(ui, 0, |ui| {
            card::card(ui, |ui| {
                width::then(
                    ui,
                    |ui| copy = key::key(ui, t(Key::IdDoCopy), Role::Secondary, can_sign).clicked(),
                    |ui, room| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            paint::line(ui, &name, Type::Page, c(C::Ink), room);
                            paint::line(ui, &kind, Type::Note, c(C::Ink2), room);
                        });
                    },
                );
            });
        });
        if self.shell.tasks.in_flight(crate::task::Kind::Keystore) {
            states::note_box(ui, t(Key::V2Encrypting));
        }
        if unseated {
            states::note_box(ui, t(Key::IdSeatEmptyNote));
        }
        // The identity: what it is, when made, the balance and the backup state (the action layer reads the disk;
        // the frame only reads fields).
        let who = match &current {
            Some((r, _)) if !r.label.trim().is_empty() => format!("{} \u{b7} {} \u{b7} {}", t(id_seat_key(seat)), t(id_kind_key(r.kind())), r.label),
            Some((r, _)) => format!("{} \u{b7} {}", t(id_seat_key(seat)), t(id_kind_key(r.kind()))),
            None => t(Key::IdNone).to_string(),
        };
        let created = match current.as_ref().map(|(r, _)| r.created.clone()) {
            Some(ref c) if c != crate::identity::NO_CREATED => c.get(0..10).unwrap_or(c).to_string(),
            _ => t(Key::IdCreatedNone).to_string(),
        };
        let mut rows: Vec<(&str, Val)> = vec![(t(Key::IdCurrent), Val::text(who)), (t(Key::IdCreated), Val::mono(created))];
        if unseated {
            rows.push((t(Key::IdAddress), Val::Mark(Mark::Warn, t(Key::IdSeatEmptyHow).to_string())));
        } else if self.shell.anchor.is_none() {
            rows.push((t(Key::IdAddress), Val::Mark(Mark::Bad, t(if current.is_some() { Key::IdKeyMissing } else { Key::SetNoKey }).to_string())));
        }
        rows.push((
            t(Key::IdGas),
            match self.chain_read() {
                Err(f) => Val::Mark(Mark::Bad, fill1(Key::SetReadFailedNow, f.human())),
                Ok(Some(Done::Chain { gas_wei: Some(w), .. })) => Val::Mark(if *w > 0 { Mark::Ok } else { Mark::Bad }, fill1(Key::SetGasSay, &eth_held(*w))),
                _ => Val::Mark(Mark::Todo, t(Key::SetNotRead).to_string()),
            },
        ));
        rows.push((
            t(Key::IdBackupState),
            match &current {
                Some((r, _)) => {
                    let k = match (r.backed_words, r.backed_file) {
                        (true, true) => Key::IdBackedBoth,
                        (true, false) => Key::IdBackedWords,
                        (false, true) => Key::IdBackedFile,
                        (false, false) => Key::IdNotBacked,
                    };
                    Val::Mark(if r.backed() { Mark::Ok } else { Mark::Warn }, t(k).to_string())
                }
                None => Val::Mark(Mark::Todo, t(Key::SetNotRead).to_string()),
            },
        ));
        let seen = self.shell.backup_seen.clone();
        rows.push((
            t(Key::IdBackupAt),
            match &seen {
                Some(b) if b.at == crate::identity::NO_BACKUP_AT => Val::Mark(Mark::Todo, t(Key::IdBackupNever).to_string()),
                Some(b) if b.exists && b.opens => Val::Mark(Mark::Ok, t(Key::IdBackupThere).to_string()),
                Some(_) => Val::Mark(Mark::Bad, t(Key::IdBackupGone).to_string()),
                None => Val::Mark(Mark::Todo, t(Key::SetNotRead).to_string()),
            },
        ));
        rows.push((t(Key::IdStore), Val::text(t(Key::IdKeybox))));
        stagger(ui, 1, |ui| card::section(ui, t(Key::IdGroup), "", |ui| card::card(ui, |ui| kv::kv(ui, &rows))));
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
        let mut set_primary = false;
        stagger(ui, 2, |ui| {
            card::section(ui, t(Key::IdPrimaryTag), t(Key::IdPrimaryFoot), |ui| {
                card::form(ui, |ui, f| {
                    if this_primary {
                        let by = match primary.as_ref().map(|(_, k)| *k) {
                            Some(crate::keybox::PrimaryKind::KeyFile) => Key::IdPrimaryByFile,
                            _ => Key::IdPrimaryByWords,
                        };
                        f.row(ui, t(Key::IdPrimaryTag), t(by), card::Value::Text(t(Key::IdPrimaryThis)));
                    } else {
                        f.row(ui, t(Key::IdPrimaryTag), "", card::Value::Text(&primary_name));
                        set_primary = f.row(ui, t(Key::IdDoSetPrimary), "", if current.is_some() && !unseated { card::Value::Nav } else { card::Value::Off }).clicked();
                    }
                });
            });
        });
        // The passcode group, in order: the passcode, the auto-lock switch, the idle time (only while on), and
        // change passcode. Changes take effect at once; nothing to save.
        let pin_set = !self.shell.vault.absent();
        let lock_on = self.shell.machine.auto_lock;
        let lock_now = self.shell.machine.auto_lock_secs;
        let lock_items: Vec<String> = crate::machine::LOCK_CHOICES.iter().map(|x| fill1(Key::MinutesN, &(x / 60).to_string())).collect();
        let lock_label = fill1(Key::MinutesN, &(lock_now / 60).to_string());
        let (mut change, mut set_pin, mut lock_pick, mut flip) = (false, false, None, false);
        stagger(ui, 3, |ui| {
            card::section(ui, t(Key::IdPin), "", |ui| {
                card::form(ui, |ui, f| {
                    if pin_set {
                        f.row(ui, t(Key::IdPin), t(Key::IdPinNote), card::Value::None);
                        let note = t(if lock_on { Key::IdAutoLockOnNote } else { Key::IdAutoLockOffNote });
                        f.row_with(ui, None, t(Key::IdAutoLock), note, card::Value::None, |ui| {
                            flip = zikaron_ui::toggle::switch(ui, lock_on, true).clicked();
                        });
                        if lock_on {
                            f.row_with(ui, None, t(Key::IdIdleFor), "", card::Value::None, |ui| {
                                let items: Vec<menu::Item> = lock_items
                                    .iter()
                                    .enumerate()
                                    .map(|(i, s)| menu::Item::Row(menu::Row { label: s, check: Some(crate::machine::LOCK_CHOICES[i] == lock_now), ..Default::default() }))
                                    .collect();
                                lock_pick = menu::menu_key(ui, "auto-lock", &lock_label, false, 150.0, &items);
                            });
                        }
                        change = f.row(ui, t(Key::IdDoChangePin), "", card::Value::Nav).clicked();
                    } else {
                        f.row(ui, t(Key::IdPin), "", card::Value::Text(t(Key::IdPinNone)));
                        f.keys(ui, |ui| set_pin = key::key(ui, t(Key::IdDoSetPin), Role::Secondary, true).clicked());
                    }
                });
            });
            if self.shell.unlocked() && crate::keybox::pin_digits_only() == Some(true) {
                hint(ui, t(Key::PinDigitsOnly));
            }
        });
        let (mut backup, mut words) = (false, false);
        stagger(ui, 4, |ui| {
            card::section(ui, t(Key::IdExportGroup), if has_words || current.is_none() { "" } else { t(Key::IdNoWordsHint) }, |ui| {
                card::form(ui, |ui, f| {
                    backup = f.row(ui, t(Key::IdDoBackup), "", if can_sign { card::Value::Nav } else { card::Value::Off }).clicked();
                    words = f.row(ui, t(Key::IdDoWords), "", if !unseated && has_words { card::Value::Nav } else { card::Value::Off }).clicked();
                });
            });
        });
        let (mut switch, mut new, mut import, mut rename) = (false, false, false, false);
        let change_note = t(if seat == crate::roles::Role::Author { Key::IdChangeAuthorHint } else { Key::IdChangeGranteeHint });
        stagger(ui, 5, |ui| {
            card::section(ui, t(Key::IdChangeFold), change_note, |ui| {
                card::form(ui, |ui, f| {
                    switch = f.row(ui, t(Key::IdDoSwitch), "", if reg.rows.len() > 1 { card::Value::Nav } else { card::Value::Off }).clicked();
                    new = f.row(ui, t(Key::IdDoNew), "", card::Value::Nav).clicked();
                    import = f.row(ui, t(Key::IdDoImport), "", card::Value::Nav).clicked();
                    rename = f.row(ui, t(Key::IdDoRename), "", if current.is_some() { card::Value::Nav } else { card::Value::Off }).clicked();
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
        }
        raw.push((t(Key::IdSeatRowAuthor), seat_addr(crate::roles::Role::Author)));
        raw.push((t(Key::IdSeatRowGrantee), seat_addr(crate::roles::Role::Grantee)));
        if let Some(b) = seen.as_ref().filter(|b| b.at != crate::identity::NO_BACKUP_AT) {
            raw.push((t(Key::IdBackupAt), Val::mono(b.at.clone())));
        }
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
        stagger(ui, 6, |ui| details_card(ui, "keys-details", &raw));
        let mut delete = false;
        stagger(ui, 7, |ui| {
            if current.is_some() {
                card::form(ui, |ui, f| delete = f.danger(ui, t(Key::IdDoDelete)).clicked());
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
        // (`deploy::same_as_row`), else custom. Only this line uses it.
        let row = s.network.as_deref().and_then(crate::deploy::named).or_else(|| crate::deploy::same_as_row(s.chain_id, s.registry, s.from_block, &s.endpoints));
        let network = match row {
            Some(d) if !bare => Val::text(network_label(d.name)),
            _ if bare => Val::Mark(Mark::Warn, t(Key::SetNetworkNone).to_string()),
            _ => Val::text(t(Key::U3Custom)),
        };
        let read = match self.chain_read() {
            // The last read failed: show this failure, not the reading before it.
            Err(f) => Val::Mark(Mark::Bad, fill1(Key::SetReadFailedNow, f.human())),
            Ok(Some(Done::Chain { sources, single_source, unanswered, .. })) => {
                let said = if *single_source { t(Key::SetSingleSource).to_string() } else { fill1(Key::SetAgreed, &sources.to_string()) };
                // Name the nodes that gave nothing, so "single source" says which one stayed silent: the first layer
                // counts them, the details name them.
                let said = if unanswered.is_empty() { said } else { format!("{said} · {}", fill1(Key::SetUnansweredCount, &unanswered.len().to_string())) };
                Val::Mark(if *single_source || !unanswered.is_empty() { Mark::Warn } else { Mark::Ok }, said)
            }
            _ => Val::Mark(Mark::Todo, t(Key::SetNotRead).to_string()),
        };
        let none = || Val::text(t(Key::SetNone));
        stagger(ui, 0, |ui| {
            card::card(ui, |ui| {
                ui.spacing_mut().item_spacing.y = tk::S3;
                kv::kv(ui, &[(t(Key::SetChain), network), (t(Key::SetChainRead), read)]);
                if let Ok(Some(Done::Chain { unanswered, .. })) = self.chain_read() {
                    if !unanswered.is_empty() {
                        let unanswered = unanswered.clone();
                        fold::fold(ui, "chain-unanswered", t(Key::SetWhatEachSaid), |ui| {
                            hint(ui, &fill1(Key::SetUnanswered, &unanswered.join(" · ")));
                        });
                    }
                }
                details(
                    ui,
                    "network-details",
                    &[
                        (t(Key::SetNodes), if s.endpoints.is_empty() { none() } else { Val::mono(s.endpoints.join("\n")) }),
                        (t(Key::SetRegistry), s.registry.map(|a| Val::mono(a.hex())).unwrap_or_else(none)),
                        (t(Key::BasisChain), s.chain_id.map(|n| Val::mono(n.to_string())).unwrap_or_else(none)),
                        (t(Key::BasisFrom), Val::mono(s.from_block.to_string())),
                    ],
                );
            });
        });
        // Auto put on chain (per home, off by default): read by the writing keys' labels and by what follows a
        // write.
        let mut flip = false;
        stagger(ui, 1, |ui| {
            card::form(ui, |ui, f| {
                f.row_with(ui, None, t(Key::SetAutoAnchor), t(Key::SetAutoAnchorSay), card::Value::None, |ui| flip = toggle::switch(ui, s.auto_anchor, true).clicked());
            });
        });
        if flip {
            self.act(Action::SetAutoAnchor { on: !s.auto_anchor }, now);
        }
        // How the command line puts records on chain through the desktop (machine-wide).
        let all = crate::machine::CliAnchor::ALL;
        let cur = all.iter().position(|x| *x == self.shell.machine.cli_anchor).unwrap_or(0);
        let cells: Vec<seg::Cell> = all.iter().map(|x| seg::Cell::from(t(cli_anchor_label(*x)))).collect();
        let mut pick = None;
        stagger(ui, 1, |ui| {
            card::form(ui, |ui, f| {
                f.row_with(ui, None, t(Key::SetCliAnchor), "", card::Value::None, |ui| pick = seg::seg(ui, "set-cli-anchor", &cells, cur));
            });
        });
        if let Some(i) = pick.filter(|i| *i != cur) {
            self.act(Action::SetCliAnchor { to: all[i] }, now);
        }
        let (mut read_chain, mut save_nodes, mut save_basis) = (false, false, false);
        stagger(ui, 2, |ui| {
            card::card(ui, |ui| {
                ui.spacing_mut().item_spacing.y = tk::S3;
                // A home without a network is set up here, by hand or from a preset (the only place to do so).
                if bare {
                    hint(ui, t(Key::SetNetworkNoneSay));
                }
                keys_row(ui, |ui| {
                    if key::key(ui, t(Key::SetEditNodes), Role::Secondary, true).clicked() {
                        self.ux.open_nodes = !self.ux.open_nodes;
                    }
                    read_chain = self.long_key(ui, t(Key::DoReadChain), Role::Secondary, true, crate::task::Kind::Chain);
                });
                self.stage_line(ui, crate::task::Kind::Chain);
                let open = motion::flag(ui.ctx(), egui::Id::new("network-editor"), self.ux.open_nodes, tk::MID);
                if open > 0.0 {
                    ui.scope(|ui| {
                        ui.multiply_opacity(open);
                        paint::rule(ui, 0.0);
                        // A preset fills the four cells from the known table; saving still goes through the two
                        // keys below.
                        if let Some(i) = field(ui, t(Key::ReadNetPreset), None, |ui| preset_menu(ui, "network-preset", self.ux.basis_preset)) {
                            self.ux.basis_preset = i;
                            if let Some(c) = preset_cells(i) {
                                self.typed.endpoints = c.endpoints;
                                self.ux.basis_chain = c.chain;
                                self.ux.basis_registry = c.registry;
                                self.ux.basis_from = c.from;
                            }
                        }
                        let (_, save) = width::line_then(ui, &mut self.typed.endpoints, t(Key::EndpointHint), true, |ui| key::key(ui, t(Key::DoSetEndpoints), Role::Secondary, true).clicked());
                        save_nodes = save;
                        card::grid(ui, "network-basis", 3, 120.0, |ui, i| {
                            let (slot, hint_key) = match i {
                                0 => (&mut self.ux.basis_chain, Key::BasisChain),
                                1 => (&mut self.ux.basis_registry, Key::SetRegistry),
                                _ => (&mut self.ux.basis_from, Key::BasisFrom),
                            };
                            let w = ui.available_width();
                            input::field(ui, slot, t(hint_key), w, input::Look { mono: true, ..Default::default() });
                        });
                        // Saving first checks the registry's code at this home's nodes (in the background); the
                        // last check's reading for these cells stays beside them.
                        save_basis = self.long_key(ui, t(Key::DoSetBasis), Role::Secondary, true, crate::task::Kind::Basis);
                        let typed = (self.ux.basis_chain.trim().parse::<u64>().ok(), crate::key::Address::parse(&self.ux.basis_registry));
                        if let Some((c, r, reading)) = self.shell.basis_read {
                            if typed == (Some(c), Some(r)) {
                                // No reading yet (no node for this chain, or the check still running): shown as not
                                // checked, never as checked.
                                let (m, k) = match reading {
                                    Some(r @ (crate::widex::Reading::Agreed(_) | crate::widex::Reading::Single)) => (Mark::Ok, r.key()),
                                    Some(crate::widex::Reading::Fingerprint) => (Mark::Bad, Key::BadgeFingerprint),
                                    Some(crate::widex::Reading::Down) => (Mark::Warn, Key::BadgeDown),
                                    None => (Mark::Warn, Key::BadgeUnchecked),
                                };
                                states::okline(ui, m, t(k));
                            }
                        }
                    });
                }
            });
        });
        if read_chain {
            self.act(Action::ReadChain, now);
        }
        if save_nodes {
            let a = Action::SetEndpoints { specs: self.typed.endpoints.clone() };
            self.act(a, now);
        }
        if save_basis {
            let a = Action::SetBasis { chain: self.ux.basis_chain.clone(), registry: self.ux.basis_registry.clone(), from_block: self.ux.basis_from.clone() };
            self.act(a, now);
        }
        stagger(ui, 3, |ui| self.set_read_nets(ui, now));
        stagger(ui, 4, |ui| card::section(ui, t(Key::SetPublish), "", |ui| card::card(ui, |ui| self.set_publish(ui, now))));
        stagger(ui, 5, |ui| card::section(ui, t(Key::SetProxy), "", |ui| self.set_proxy(ui, now)));
    }

    /// The proxy (machine-wide): follow the system, none, or a typed address, parsed the way the transport
    /// parses it and refused by name before saving.
    fn set_proxy(&mut self, ui: &mut egui::Ui, now: f64) {
        use crate::machine::proxy::{NONE, SYSTEM};
        ui.spacing_mut().item_spacing.y = tk::S3;
        let written = self.shell.machine.proxy.clone();
        let cur = match written.as_deref().map(str::trim) {
            None | Some(SYSTEM) => 0,
            Some(NONE) => 1,
            Some(_) => 2,
        };
        if cur == 2 && !self.ux.proxy_seeded {
            self.typed.proxy = written.clone().unwrap_or_default();
            self.ux.proxy_seeded = true;
        }
        let shown = if self.ux.proxy_manual { 2 } else { cur };
        let cells: Vec<seg::Cell> = [Key::ProxySystem, Key::ProxyOff, Key::ProxyManual].iter().map(|k| seg::Cell::from(t(*k))).collect();
        let mut pick = None;
        card::form(ui, |ui, f| {
            f.row_with(ui, None, t(Key::SetProxy), "", card::Value::None, |ui| pick = seg::seg(ui, "set-proxy", &cells, shown));
        });
        match pick {
            Some(2) => self.ux.proxy_manual = true,
            Some(i) if i != shown => {
                self.ux.proxy_manual = false;
                self.act(Action::SetProxy { choice: if i == 0 { SYSTEM } else { NONE }.to_string() }, now);
            }
            _ => {}
        }
        if self.ux.proxy_manual || cur == 2 {
            let typed = self.typed.proxy.trim().to_string();
            let differs = cur != 2 || Some(typed.as_str()) != written.as_deref();
            let (resp, hit) = width::line_then(ui, &mut self.typed.proxy, t(Key::ProxyHint), true, |ui| key::key(ui, t(Key::ReadNetSave), Role::Secondary, differs).clicked());
            match (!typed.is_empty()).then(|| zikaron_net::proxy_of(&typed).err()).flatten() {
                Some(zikaron_net::NotAProxy::Credentials) => states::okline(ui, Mark::Bad, &fill1(Key::TailProxyCredentials, &typed)),
                Some(zikaron_net::NotAProxy::Shape) => states::okline(ui, Mark::Bad, &fill1(Key::TailProxyShape, &typed)),
                None => {}
            }
            // Saving goes to the action layer even when the address does not parse: it refuses by name there and
            // nothing is written.
            if (resp.lost_focus() && differs && !typed.is_empty()) || hit {
                self.act(Action::SetProxy { choice: typed }, now);
                if self.shell.machine.proxy.as_deref().map(|p| p != SYSTEM && p != NONE).unwrap_or(false) {
                    self.ux.proxy_manual = false;
                }
            }
        }
    }

    /// The read-only networks (machine-wide): one row each with its name and reading; a row opens to its four
    /// cells and three keys. "Add" starts one new row (only one unsaved at a time), from a preset or by hand.
    fn set_read_nets(&mut self, ui: &mut egui::Ui, now: f64) {
        let nets = self.shell.read_nets.clone().unwrap_or_default();
        let reads = self.shell.net_reads.clone();
        let rn = &mut self.ux.readnet;
        // Forget rows of networks no longer in the table.
        rn.open.retain(|k| nets.iter().any(|n| n.is(k.0, &k.1)));
        rn.drafts.retain(|(k, _)| nets.iter().any(|n| n.is(k.0, &k.1)));
        let mut act: Option<Action> = None;
        let mut add = false;
        let unsaved = rn.new.is_some();
        ui.horizontal(|ui| {
            card::group_title(ui, t(Key::ReadNetTitle));
            if !unsaved {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| add = key::key(ui, t(Key::ReadNetAdd), Role::Secondary, true).clicked());
            }
        });
        if nets.is_empty() && !unsaved {
            return self.read_nets_added(add);
        }
        card::card(ui, |ui| {
            ui.spacing_mut().item_spacing.y = tk::S2;
            for (i, n) in nets.iter().enumerate() {
                if i > 0 {
                    paint::rule(ui, 0.0);
                }
                let k = (n.chain_id, n.registry);
                let open = rn.open.contains(&k);
                let badge = reads.iter().find(|(c, r, _)| *c == k.0 && *r == k.1).map(|(_, _, x)| (t(x.key()), read_tone(*x)));
                if net_row(ui, &format!("{}-{}", k.0, k.1.hex()), &n.name(), badge, open).clicked() {
                    if open {
                        rn.open.retain(|x| *x != k);
                        rn.drafts.retain(|(x, _)| *x != k);
                        if rn.armed == Some(k) {
                            rn.armed = None;
                        }
                    } else {
                        rn.open.push(k);
                        rn.drafts.push((k, Draft::of(n)));
                    }
                }
                if !rn.open.contains(&k) {
                    continue;
                }
                let Some(d) = rn.drafts.iter_mut().find(|(x, _)| *x == k).map(|(_, d)| d) else { continue };
                ui.push_id(("readnet-body", k.0, k.1.hex()), |ui| net_cells(ui, d, false));
                keys_row(ui, |ui| {
                    if rn.armed == Some(k) {
                        paint::text(ui, t(Key::ReadNetRemoveAsk), Type::Body, c(C::Ink));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked() {
                                rn.armed = None;
                            }
                            if page::Guide::key(ui, t(Key::KitRemove), true).clicked() {
                                act = Some(Action::RemoveReadNetwork { chain: k.0, registry: k.1.hex() });
                            }
                        });
                    } else {
                        if key::key(ui, t(Key::ReadNetSave), Role::Primary, true).clicked() {
                            act = Some(d.save(Some(k)));
                        }
                        if key::key(ui, t(Key::StageReadChain), Role::Secondary, true).clicked() {
                            act = Some(Action::ReadReadNetwork { chain: k.0, registry: k.1.hex() });
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if page::Guide::key(ui, t(Key::KitRemove), true).clicked() {
                                rn.armed = Some(k);
                            }
                        });
                    }
                });
            }
            if let Some(d) = rn.new.as_mut() {
                if !nets.is_empty() {
                    paint::rule(ui, 0.0);
                }
                net_row(ui, "readnet-new", t(Key::ReadNetNew), None, true);
                ui.push_id("readnet-new-body", |ui| net_cells(ui, d, true));
                let mut drop = false;
                keys_row(ui, |ui| {
                    if key::key(ui, t(Key::ReadNetSave), Role::Primary, true).clicked() {
                        act = Some(d.save(None));
                    }
                    if key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked() {
                        drop = true;
                    }
                });
                if drop {
                    rn.new = None;
                }
            }
        });
        if let Some(a) = act {
            let was = match &a {
                Action::SaveReadNetwork { was, .. } => Some(was.clone()),
                _ => None,
            };
            let removed = match &a {
                Action::RemoveReadNetwork { chain, registry } => crate::key::Address::parse(registry).map(|r| (*chain, r)),
                _ => None,
            };
            if let Applied::ReadNets(_) = self.act(a, now) {
                let rn = &mut self.ux.readnet;
                match was {
                    // Saved: the row closes (a new row leaves the drafts).
                    Some(None) => rn.new = None,
                    Some(Some((c, r))) => {
                        if let Some(r) = crate::key::Address::parse(&r) {
                            rn.open.retain(|x| *x != (c, r));
                            rn.drafts.retain(|(x, _)| *x != (c, r));
                        }
                    }
                    None => {}
                }
                if let Some(k) = removed {
                    rn.open.retain(|x| *x != k);
                    rn.drafts.retain(|(x, _)| *x != k);
                    rn.armed = None;
                }
            }
        }
        self.read_nets_added(add);
    }

    /// "Add": one new, unsaved, open row.
    fn read_nets_added(&mut self, add: bool) {
        if add && self.ux.readnet.new.is_none() {
            self.ux.readnet.new = Some(Draft::default());
        }
    }

    /// From the verify page, for a kit naming a network not yet added: open the settings' network page with a
    /// new row holding the kit's chain, registry and start block (replacing any unsaved row); the user fills
    /// in the nodes.
    pub(super) fn add_stated_network(&mut self, at: &crate::kitsindex::AnchoredOn, now: f64) {
        self.ux.readnet.new = Some(Draft {
            preset: 0,
            name: String::new(),
            chain: at.chain_id.to_string(),
            registry: at.registry.clone(),
            from: at.from_block.to_string(),
            nodes: String::new(),
        });
        self.go(Place::Settings(Section::Network), now);
    }

    /// The publish address (https only, checked at once) and "check publication" against a local kit, with a
    /// sentence for each of three outcomes.
    fn set_publish(&mut self, ui: &mut egui::Ui, now: f64) {
        ui.spacing_mut().item_spacing.y = tk::S3;
        if !self.ux.publish_seeded {
            self.typed.publish = self.shell.settings.publish.clone().unwrap_or_default();
            self.ux.publish_seeded = true;
        }
        let typed = self.typed.publish.trim().to_string();
        let differs = typed != self.shell.settings.publish.clone().unwrap_or_default();
        let (resp, hit) = width::line_then(ui, &mut self.typed.publish, "https://\u{2026}", true, |ui| key::key(ui, t(Key::DoSetPublish), Role::Secondary, differs).clicked());
        let save = (resp.lost_focus() && differs) || hit;
        let shape = (!typed.is_empty()).then(|| crate::fetchx::base_of(&typed).err()).flatten();
        match &shape {
            Some(f) => states::okline(ui, Mark::Bad, f.human()),
            None => hint(ui, t(Key::SetPublishNote)),
        }
        if save && shape.is_none() {
            self.act(Action::SetPublish { url: typed.clone() }, now);
        }
        if self.shell.settings.publish.is_none() {
            return;
        }
        if self.typed.publish_local.trim().is_empty() {
            if let Some(p) = self.shell.home.as_ref().and_then(|h| crate::kitx::latest_kit(&h.dir(crate::home::Slot::Kits))) {
                self.typed.publish_local = p.display().to_string();
            }
        }
        Self::place_row(ui, Key::SetPublishLocal, &mut self.typed.publish_local);
        if self.long_key(ui, t(Key::DoCheckPublished), Role::Secondary, true, crate::task::Kind::Publish) {
            let a = Action::CheckPublished { local: self.typed.publish_local.clone() };
            self.act(a, now);
        }
        self.stage_line(ui, crate::task::Kind::Publish);
        if self.shell.tasks.in_flight(crate::task::Kind::Publish) {
            return;
        }
        if let Some(f) = self.shell.failed.get(&crate::task::Kind::Publish) {
            states::err_box(ui, "publish-err", t(Key::PublishUnreachable), f.human(), t(Key::U3RawError), &f.raw());
        } else if let Some((_, r)) = self.shell.published.as_ref() {
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
    }

    fn set_notify(&mut self, ui: &mut egui::Ui, now: f64) {
        let s = self.shell.settings.clone();
        let grantee = s.role == crate::roles::Role::Grantee;
        let every = |n: u64| if n == 0 { t(Key::SetEveryOff).to_string() } else { fill1(Key::SetEvery, &n.to_string()) };
        let (title, value) = if grantee { (t(Key::SetReview), every(s.review_every)) } else { (t(Key::PageAudit), every(s.audit_every)) };
        let mut save = false;
        stagger(ui, 0, |ui| {
            card::form(ui, |ui, f| {
                f.row(ui, title, "", card::Value::Text(&value));
                f.keys(ui, |ui| {
                    if key::key(ui, t(Key::SetCadenceEdit), Role::Secondary, true).clicked() {
                        self.ux.open_cadence = !self.ux.open_cadence;
                    }
                });
                if self.ux.open_cadence {
                    f.free(ui, |ui| {
                        let slot = if grantee { &mut self.ux.review_every } else { &mut self.ux.audit_every };
                        let label = t(if grantee { Key::DoSetReviewEvery } else { Key::DoSetAuditEvery });
                        let (_, hit) = width::line_then(ui, slot, t(if grantee { Key::SetReview } else { Key::AuditEvery }), true, |ui| key::key(ui, label, Role::Secondary, true).clicked());
                        save = hit;
                    });
                }
            });
        });
        if save {
            if grantee {
                let a = Action::SetReviewEvery { secs: self.ux.review_every.clone() };
                self.act(a, now);
            } else {
                match self.ux.audit_every.trim().parse::<u64>() {
                    Ok(n) => {
                        self.act(Action::SetAuditEvery { secs: n }, now);
                    }
                    Err(_) => self.toasts.say(t(Key::U3EveryMustBeNumber), Tone::Bad, now),
                }
            }
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
        // measurement; the frame reads no disk).
        let usage = match self.shell.archive.as_ref() {
            Some(a) => Val::Mark(if a.bytes > self.shell.settings.cap_bytes { Mark::Bad } else { Mark::Ok }, fill2(Key::SetUsageHuman, &size_say(a.bytes), &size_say(self.shell.settings.cap_bytes))),
            None => Val::Mark(Mark::Todo, t(Key::SetNotRead).to_string()),
        };
        let ids = self.shell.identities.as_ref().map(|r| r.rows.len()).unwrap_or(0);
        let content = match self.shell.archive.as_ref() {
            Some(a) if a.skipped == 0 => Val::text(crate::lang::filln(Key::BackupContent, &[&ids.to_string(), &a.items.to_string(), &a.records.to_string()])),
            Some(a) => Val::Mark(Mark::Warn, fill2(Key::SetStraysSay, &a.items.to_string(), &a.skipped.to_string())),
            None => Val::Mark(Mark::Todo, t(Key::SetNotRead).to_string()),
        };
        stagger(ui, 0, |ui| {
            card::card(ui, |ui| kv::kv(ui, &[(t(Key::DataContent), content), (t(Key::DataProtect), Val::Mark(Mark::Ok, t(Key::DataProtectOn).to_string())), (t(Key::SetUsage), usage)]))
        });
        // The whole-machine backup: when the last one was written, what came after it, export, restore.
        let last = self.shell.machine.backup.clone();
        let behind = self.backup_behind();
        let (mut export, mut restore) = (false, false);
        stagger(ui, 1, |ui| {
            card::section(ui, t(Key::WizBackupTitle), t(Key::BackupRule), |ui| {
                card::form(ui, |ui, f| {
                    match &last {
                        Some(b) => f.row(ui, t(Key::DataBackupLast), "", card::Value::Mono(&crate::when::when(b.at))),
                        None => f.row(ui, t(Key::DataBackupLast), "", card::Value::Text(t(Key::DataBackupNever))),
                    };
                    match (&last, behind) {
                        (Some(_), Some(n)) if n > 0 => {
                            let say = fill1(Key::U3EntriesCount, &n.to_string());
                            f.row_with(ui, Some(Mark::Warn), t(Key::IdNotBacked), "", card::Value::Text(&say), |_| ());
                        }
                        (Some(_), Some(_)) => {
                            f.row(ui, t(Key::IdNotBacked), "", card::Value::Text(t(Key::Nothing)));
                        }
                        _ => {
                            f.row(ui, t(Key::IdNotBacked), "", card::Value::Text("\u{2014}"));
                        }
                    }
                    export = f.row(ui, t(Key::DoExportBackup), "", if self.shell.unlocked() { card::Value::Nav } else { card::Value::Off }).clicked();
                    restore = f.row(ui, t(Key::DoRestoreBackup), "", card::Value::Nav).clicked();
                });
            });
        });
        if export {
            self.bk_open(Bk::Export);
        }
        if restore {
            self.bk_open(Bk::Restore(RestoreFrom::Settings));
        }
        // The ledger mirror: an export of the recorder's ledger; it does not restore this machine.
        if self.shell.settings.role == crate::roles::Role::Author {
            let mut mirror = false;
            stagger(ui, 2, |ui| {
                card::section(ui, t(Key::PageMirror), "", |ui| {
                    card::form(ui, |ui, f| mirror = f.row(ui, t(Key::DoExportMirror), "", card::Value::Nav).clicked());
                });
            });
            if mirror {
                self.bk_open(Bk::Mirror);
            }
        }
        // On an empty seat, keys that need a home or a key are disabled, as on the identity page
        // (`Shell::seat_unseated`).
        let seat_ok = !self.shell.seat_unseated();
        if !seat_ok {
            states::note_box(ui, t(Key::IdSeatEmptyNote));
        }
        let grantee = self.shell.settings.role == crate::roles::Role::Grantee;
        let (mut measure, mut open_home, mut import_dir) = (false, false, false);
        stagger(ui, 3, |ui| {
            card::group_title(ui, t(Key::SetHomeOfSeat));
            card::card(ui, |ui| {
                ui.spacing_mut().item_spacing.y = tk::S3;
                keys_row(ui, |ui| {
                    if key::key(ui, t(Key::SetChangeHome), Role::Secondary, seat_ok).clicked() {
                        self.ux.open_home = !self.ux.open_home;
                    }
                    measure = self.long_key(ui, t(Key::DoMeasure), Role::Secondary, seat_ok, crate::task::Kind::Archive);
                    if grantee && key::key(ui, t(Key::V2ImportGrantDir), Role::Secondary, seat_ok).clicked() {
                        self.ux.open_grant_dir = !self.ux.open_grant_dir;
                    }
                });
                self.stage_line(ui, crate::task::Kind::Archive);
                if self.ux.open_home && seat_ok {
                    paint::rule(ui, 0.0);
                    pick_path(ui, &mut self.typed.home, crate::platform::Pick::Folder);
                    open_home = page::Page::new().primary_with(ui, t(Key::DoOpenHome), Self::landing_ok(&self.typed.home)).1.clicked();
                }
                if grantee && self.ux.open_grant_dir && seat_ok {
                    paint::rule(ui, 0.0);
                    pick_path(ui, &mut self.typed.grant_dir, crate::platform::Pick::Folder);
                    import_dir = key::key(ui, t(Key::V2ImportGrantDirGo), Role::Secondary, !self.typed.grant_dir.trim().is_empty()).clicked();
                }
            });
        });
        // Display only, for the recorder's two lists: hide what a deletion leaves on this machine only.
        if self.shell.settings.role == crate::roles::Role::Author {
            let on = self.shell.settings.hide_local_deletions;
            let mut flip = false;
            stagger(ui, 3, |ui| {
                card::form(ui, |ui, f| {
                    f.row_with(ui, None, t(Key::SetHideLocalDeletions), "", card::Value::None, |ui| flip = toggle::switch(ui, on, seat_ok).clicked());
                });
            });
            if flip {
                self.act(Action::SetHideLocalDeletions { on: !on }, now);
            }
        }
        self.set_cli_path(ui, now);
        if measure {
            self.act(Action::Measure, now);
        }
        if open_home {
            let a = Action::ChangeHome { root: self.typed.home.clone() };
            self.act(a, now);
            // Another home may have chosen another language: decide again next frame.
            self.ux.lang_applied = false;
        }
        if import_dir {
            let a = Action::ImportGrantDir { dir: self.typed.grant_dir.clone() };
            self.act(a, now);
        }
        let (mut set_cap, mut migrate, mut adopt) = (false, false, false);
        let root = self.shell.home.as_ref().map(|h| h.root().display().to_string()).unwrap_or_else(|| t(Key::HomeNotOpen).to_string());
        let lock = match self.shell.lock.as_ref() {
            None => t(Key::HomeNoLock).to_string(),
            Some(l) => format!("{} \u{b7} {}", l.mode().as_str(), l.holder()),
        };
        let miss = self.shell.home.as_ref().map(|h| h.missing()).unwrap_or_default();
        let mut reconcile = false;
        let mut recheck = false;
        stagger(ui, 4, |ui| {
            card::card(ui, |ui| {
                ui.spacing_mut().item_spacing.y = tk::S2;
                fold::fold(ui, "data-more", t(Key::U3MoreOptions), |ui| {
                    ui.spacing_mut().item_spacing.y = tk::S3;
                    field(ui, t(Key::SetCapLabel), None, |ui| {
                        let (_, hit) = width::line_then(ui, &mut self.typed.cap, "", true, |ui| key::key(ui, t(Key::DoSetCap), Role::Secondary, seat_ok).clicked());
                        set_cap = hit;
                    });
                    let asked = path_row(ui, t(Key::MigrateLabel), &self.typed.migrate, t(Key::PickFolder), t(Key::PickNone));
                    {
                        if let Some(p) = path_answer(ui.ctx(), egui::Id::new("zikaron-path-migrate"), asked, crate::platform::Pick::Folder) {
                            self.typed.migrate = p;
                            // Working out where the move would land walks the disk, so it is done once, when the
                            // folder is picked; the frame only reads it.
                            self.ux.migrate_landing = Some(crate::home::choose(&crate::home::Kind::Home, std::path::Path::new(self.typed.migrate.trim())));
                        }
                    }
                    // The landing place, with why it is not the one picked.
                    let picked = self.ux.migrate_landing.clone();
                    if let Some(p) = picked.as_ref() {
                        if let Some(k) = p.why.say_ahead() {
                            hint(ui, &fill1(k, &width::file_name(&p.at.display().to_string())));
                        }
                    }
                    // While the whole tree is copied (`Kind::Migrate`) the key shows a spinner and ignores presses.
                    let moving = if self.shell.tasks.in_flight(crate::task::Kind::Migrate) { Phase::Busy { frac: None } } else { Phase::Idle };
                    let can = seat_ok && picked.as_ref().map(|p| Self::landing_ok(&p.at.display().to_string())).unwrap_or(false);
                    migrate = key::show(ui, key::Key::new(t(Key::DoMigrate), Role::Secondary).enabled(can).phase(moving).busy_text(t(Key::SetMigrating))).clicked();
                    field(ui, t(Key::AdoptGroup), None, |ui| pick_path(ui, &mut self.typed.adopt, crate::platform::Pick::Folder));
                    adopt = key::key(ui, t(Key::DoAdopt), Role::Secondary, seat_ok && !self.typed.adopt.trim().is_empty()).clicked();
                    // After adopting a ledger, reconcile once: only a match gives the pen back.
                    let pen = if self.shell.pen == Pen::Granted { Val::Mark(Mark::Ok, t(Key::SetPenOpen).to_string()) } else { Val::Mark(Mark::Warn, t(Key::SetPenHeld).to_string()) };
                    let last = match &self.shell.reconciled {
                        Some((label, _, n)) => Val::text(format!("{} \u{b7} {}", label_human(label), fill1(Key::U3EntriesCount, &n.to_string()))),
                        None => Val::text(t(Key::NotRun)),
                    };
                    kv::kv(ui, &[(t(Key::SetPen), pen), (t(Key::LastAudit), last)]);
                    reconcile = self.long_key(ui, t(Key::DoReconcile), Role::Secondary, true, crate::task::Kind::Reconcile);
                    self.stage_line(ui, crate::task::Kind::Reconcile);
                    // Anchors that several nodes already confirmed alike are not queried again (`checkedx`); this
                    // drops that record, so the next sync queries every anchor.
                    hint(ui, t(Key::RecheckAllNote));
                    recheck = key::key(ui, t(Key::DoRecheckAll), Role::Secondary, true).clicked();
                });
                details(
                    ui,
                    "data-details",
                    &[
                        (t(Key::SetHomeOfSeat), Val::mono(root.clone())),
                        (t(Key::DataEncrypt), Val::mono(t(Key::DataEncryptHow).to_string())),
                        (t(Key::DataBackupFile), last.as_ref().map(|b| Val::mono(b.path.clone())).unwrap_or_else(|| Val::text("\u{2014}"))),
                        (t(Key::DataBackupHow), Val::mono(t(Key::DataBackupHowSay).to_string())),
                        (t(Key::HomeWhere), Val::text(t(Key::HomeWhereNote))),
                        (t(Key::HomeInstance), Val::mono(lock.clone())),
                        (t(Key::HomeMissingRooms), Val::mono(if miss.is_empty() { t(Key::Nothing).to_string() } else { miss.join(" ") })),
                    ],
                );
            });
        });
        if set_cap {
            match self.typed.cap.trim().parse::<u64>() {
                Ok(n) => {
                    self.act(Action::SetCap { bytes: n }, now);
                }
                Err(_) => self.toasts.say(t(Key::CapMustBeNumber), Tone::Bad, now),
            }
        }
        if migrate {
            if let Some(p) = self.ux.migrate_landing.clone() {
                let a = Action::MigrateHome { to: p.at.display().to_string() };
                self.act(a, now);
                // A picked place is used once: after the move it is no longer empty.
                self.typed.migrate.clear();
                self.ux.migrate_landing = None;
            }
        }
        if adopt {
            let a = Action::Adopt { dir: self.typed.adopt.clone() };
            self.act(a, now);
        }
        if reconcile {
            self.act(Action::Reconcile, now);
        }
        if recheck {
            self.act(Action::RecheckAll, now);
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
        stagger(ui, 0, |ui| card::card(ui, |ui| kv::kv(ui, &[(t(Key::Version), Val::mono(format!("{} {}", t(Key::AppName), env!("CARGO_PKG_VERSION"))))])));
        let points = self.checklist();
        stagger(ui, 1, |ui| {
            card::section(ui, t(Key::SetMissing), "", |ui| {
                card::form(ui, |ui, f| {
                    for (m, name, _) in &points {
                        f.row_with(ui, Some(*m), name, "", card::Value::None, |ui| {
                            if *m != Mark::Ok {
                                mark::pill(ui, t(Key::SetPending), PillTone::Warn);
                            }
                        });
                    }
                });
            });
        });
        let mut again = false;
        stagger(ui, 2, |ui| {
            // Disabled on an empty seat, as on the identity page.
            keys_row(ui, |ui| again = key::key(ui, t(Key::DoWizardAgain), Role::Secondary, !self.shell.seat_unseated()).clicked());
        });
        if again {
            let step = self.progress().first_open().unwrap_or(crate::nav::Step::Pin);
            self.wizard_open(step);
        }
        let mut self_check = false;
        stagger(ui, 3, |ui| {
            card::card(ui, |ui| {
                fold::fold(ui, "about-details", t(Key::SetEvidence), |ui| {
                    ui.spacing_mut().item_spacing.y = tk::S3;
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
                    let last = match &self.shell.last {
                        None => t(Key::NotRun).to_string(),
                        Some(x) => format!("{} \u{b7} {} \u{b7} {}", x.found(), x.faces.iter().map(|f| f.bytes).sum::<u64>(), x.marks),
                    };
                    kv::kv(
                        ui,
                        &[
                            (t(Key::SetCores), Val::mono(crate::sign::domains().join(" \u{b7} "))),
                            (t(Key::BuildKind), Val::mono(build_kind().to_string())),
                            (t(Key::FontGroup), Val::mono(fonts.join(" / "))),
                            (t(Key::TraceSink), Val::mono(sink)),
                            (t(Key::TaskInFlight), Val::mono(if flying.is_empty() { t(Key::Nothing).to_string() } else { flying.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(" ") })),
                            (t(Key::TaskLastCheck), Val::mono(last)),
                        ],
                    );
                    self_check = self.long_key(ui, t(Key::DoSelfCheck), Role::Secondary, true, crate::task::Kind::SelfCheck);
                    self.stage_line(ui, crate::task::Kind::SelfCheck);
                });
            });
        });
        // The third-party licences embedded in the binary (generated from Cargo.lock at build time): the crates,
        // then each licence text; only the lines in view are laid out.
        stagger(ui, 4, |ui| {
            card::card(ui, |ui| {
                fold::fold(ui, "about-notices", t(Key::SetNotices), |ui| {
                    ui.spacing_mut().item_spacing.y = tk::S3;
                    hint(ui, &fill1(Key::SetNoticesCount, &crate::about::count().to_string()));
                    let lines = crate::about::notice_lines();
                    // One row height for drawing each line (its line height set to it) and for the scroll area's
                    // row placement, with no item spacing: rows counted and rows drawn share one pitch, so the view
                    // fills to the bottom.
                    let row_h = Type::MonoSmall.line();
                    ui.scope(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        // Scroll both ways so a licence line is never cut short.
                        egui::ScrollArea::both().id_salt("about-notices-text").max_height(320.0).auto_shrink([false, true]).show_rows(ui, row_h, lines.len(), |ui, range| {
                            for l in &lines[range] {
                                ui.add(egui::Label::new(egui::RichText::new(*l).font(Type::MonoSmall.font()).color(c(C::Ink2)).line_height(Some(row_h))).wrap_mode(egui::TextWrapMode::Extend));
                            }
                        });
                    });
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

/// What the settings page keeps for the read-only networks while they are edited (interface only).
#[derive(Default)]
pub(super) struct ReadNetUx {
    /// The rows open now (chain id, registry).
    open: Vec<(u64, crate::key::Address)>,
    /// What is typed into each open row.
    drafts: Vec<((u64, crate::key::Address), Draft)>,
    /// The one new row not yet saved.
    new: Option<Draft>,
    /// The row whose "remove" asks its question.
    armed: Option<(u64, crate::key::Address)>,
}

/// The cells of one row as typed.
#[derive(Clone, Default)]
pub(super) struct Draft {
    /// Which preset filled it (0: by hand).
    preset: usize,
    name: String,
    chain: String,
    registry: String,
    from: String,
    nodes: String,
}

/// The preset menu of a network editor (the main network's and a new read-only network's): "by hand" first,
/// then every row of the known deployments table (`deploy::KNOWN`), in order.
pub(super) fn preset_menu(ui: &mut egui::Ui, salt: &str, at: usize) -> Option<usize> {
    let labels: Vec<String> = std::iter::once(t(Key::U3Custom).to_string()).chain(crate::deploy::KNOWN.iter().map(|d| t(d.label).to_string())).collect();
    let at = at.min(labels.len() - 1);
    let items: Vec<menu::Item> = labels.iter().enumerate().map(|(i, l)| menu::Item::Row(menu::Row { label: l, check: Some(i == at), ..Default::default() })).collect();
    menu::menu_key(ui, salt, &labels[at], false, 160.0, &items)
}

/// The cells a preset fills (`None` for "by hand", which keeps what was typed).
pub(super) fn preset_cells(i: usize) -> Option<crate::deploy::Cells> {
    i.checked_sub(1).and_then(|k| crate::deploy::KNOWN.get(k)).map(|d| d.cells())
}

impl Draft {
    fn of(n: &crate::readnets::Net) -> Draft {
        Draft {
            preset: 0,
            name: n.name.clone().unwrap_or_default(),
            chain: n.chain_id.to_string(),
            registry: n.registry.hex(),
            from: n.from_block.to_string(),
            nodes: n.nodes.join("\n"),
        }
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

/// One network's row: the caret (turned a quarter when open), its name, and its reading on the right.
fn net_row(ui: &mut egui::Ui, salt: &str, name: &str, badge: Option<(&str, PillTone)>, open: bool) -> egui::Response {
    use zikaron_ui::icons;
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, tk::FORM_ROW_H), egui::Sense::click());
    let id = ui.id().with(("readnet-row", salt));
    let turn = motion::flag(ui.ctx(), id, open, tk::MID) * std::f32::consts::FRAC_PI_2;
    let p = ui.painter().clone();
    icons::draw_glyph_turned(&p, Glyph::Fwd, egui::Rect::from_center_size(egui::pos2(rect.left() + 6.0, rect.center().y), egui::vec2(12.0, 12.0)), c(C::Ink3), turn);
    let mut right = rect.right();
    if let Some((s, tone)) = badge {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::right_to_left(egui::Align::Center)));
        right = mark::pill(&mut child, s, tone).rect.left() - tk::S2;
    }
    paint::at(&p, ui, egui::pos2(rect.left() + 20.0, rect.center().y), egui::Align2::LEFT_CENTER, name, Type::Body, c(C::Ink), (right - rect.left() - 20.0).max(0.0));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The cells of an open row: the preset (new rows only), the name (fixed for a chain the name table knows),
/// chain id, registry, start block, nodes.
fn net_cells(ui: &mut egui::Ui, d: &mut Draft, fresh: bool) {
    ui.spacing_mut().item_spacing.y = tk::S3;
    if fresh {
        if let Some(i) = field(ui, t(Key::ReadNetPreset), None, |ui| preset_menu(ui, "readnet-preset", d.preset)) {
            d.preset = i;
            if let Some(c) = preset_cells(i) {
                d.chain = c.chain;
                d.registry = c.registry;
                d.from = c.from;
                d.nodes = c.nodes;
            }
        }
    }
    let known = d.chain.trim().parse::<u64>().ok().and_then(crate::readnets::known_name);
    field(ui, t(Key::ReadNetName), None, |ui| match known {
        Some(k) => paint::text(ui, t(k), Type::Body, c(C::Ink2)),
        None => input::line(ui, &mut d.name, ""),
    });
    field(ui, t(Key::BasisChain), None, |ui| input::mono(ui, &mut d.chain, ""));
    field(ui, t(Key::SetRegistry), None, |ui| input::mono(ui, &mut d.registry, ""));
    field(ui, t(Key::BasisFrom), None, |ui| input::mono(ui, &mut d.from, ""));
    field(ui, t(Key::SetNodes), None, |ui| input::area(ui, &mut d.nodes, 2));
}
