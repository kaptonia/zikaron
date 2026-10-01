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
        // The identity: what it is, when made, the balance and the backup state (the facts on disk are read by
        // the action layer; the frame reads fields).
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
            match &self.shell.chain {
                Some(Done::Chain { gas_wei: Some(w), .. }) => Val::Mark(if *w > 0 { Mark::Ok } else { Mark::Bad }, fill1(Key::SetGasSay, &eth(*w))),
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
        // The primary identity: the only one that recovers the passcode. This one, or another one with "set as
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
        // The passcode group, four rows in order: the passcode, the auto-lock switch, the idle time (only while
        // on), change passcode. Changing takes effect at once; nothing to save.
        let pin_set = !matches!(self.shell.vault, crate::keybox::State::Absent);
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
        // Details: the addresses, the backup file, the derivation paths (a recovery-phrase identity only),
        // the identity list file, and which domains this identity signs (from the seat × domain table).
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
        let network = match &s.network {
            Some(n) => Val::text(fill1(Key::SetNetworkFromMachine, &network_label(n))),
            None if bare => Val::Mark(Mark::Warn, t(Key::SetNetworkNone).to_string()),
            None => Val::text(t(Key::U3Custom)),
        };
        let read = match &self.shell.chain {
            Some(Done::Chain { sources, single_source, .. }) => {
                Val::Mark(if *single_source { Mark::Warn } else { Mark::Ok }, if *single_source { t(Key::SetSingleSource).to_string() } else { fill1(Key::SetAgreed, &sources.to_string()) })
            }
            _ => Val::Mark(Mark::Todo, t(Key::SetNotRead).to_string()),
        };
        let none = || Val::text(t(Key::SetNone));
        stagger(ui, 0, |ui| {
            card::card(ui, |ui| {
                ui.spacing_mut().item_spacing.y = tk::S3;
                kv::kv(ui, &[(t(Key::SetChain), network), (t(Key::SetChainRead), read)]);
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
        // Auto put on chain (per home, off by default): the labels of the writing keys and what follows a
        // write both read it.
        let mut flip = false;
        stagger(ui, 1, |ui| {
            card::form(ui, |ui, f| {
                f.row_with(ui, None, t(Key::SetAutoAnchor), t(Key::SetAutoAnchorSay), card::Value::None, |ui| flip = toggle::switch(ui, s.auto_anchor, true).clicked());
            });
        });
        if flip {
            self.act(Action::SetAutoAnchor { on: !s.auto_anchor }, now);
        }
        let (mut use_machine, mut read_chain, mut save_nodes, mut save_basis) = (false, false, false, false);
        stagger(ui, 2, |ui| {
            card::card(ui, |ui| {
                ui.spacing_mut().item_spacing.y = tk::S3;
                keys_row(ui, |ui| {
                    // Older homes (settings present, the three fields empty) join the network this Mac chose.
                    if bare {
                        use_machine = key::key(ui, t(Key::DoUseMachineNetwork), Role::Secondary, true).clicked();
                    }
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
                        save_basis = key::key(ui, t(Key::DoSetBasis), Role::Secondary, true).clicked();
                    });
                }
            });
        });
        if use_machine {
            self.act(Action::UseMachineNetwork, now);
        }
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
        stagger(ui, 3, |ui| card::section(ui, t(Key::SetPublish), "", |ui| card::card(ui, |ui| self.set_publish(ui, now))));
    }

    /// The publish address (https only, said at once) and "check publication" against a local kit, with a
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
    /// ledger". Nothing without the mark.
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
        if pick {
            if let Some(p) = crate::platform::choose_path(crate::platform::Pick::File) {
                self.typed.fetch_from = p;
            }
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
        // On an empty seat the keys that need a home or a key are off, for the same reason as the identity
        // page (`Shell::seat_unseated`).
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
        if measure {
            self.act(Action::Measure, now);
        }
        if open_home {
            let a = Action::OpenHome { root: self.typed.home.clone() };
            self.act(a, now);
            // Another home may have chosen another language: decided again next frame.
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
        stagger(ui, 4, |ui| {
            card::card(ui, |ui| {
                ui.spacing_mut().item_spacing.y = tk::S2;
                fold::fold(ui, "data-more", t(Key::U3MoreOptions), |ui| {
                    ui.spacing_mut().item_spacing.y = tk::S3;
                    field(ui, t(Key::SetCapLabel), None, |ui| {
                        let (_, hit) = width::line_then(ui, &mut self.typed.cap, "", true, |ui| key::key(ui, t(Key::DoSetCap), Role::Secondary, seat_ok).clicked());
                        set_cap = hit;
                    });
                    if path_row(ui, t(Key::MigrateLabel), &self.typed.migrate, t(Key::PickFolder), t(Key::PickNone)) {
                        if let Some(p) = crate::platform::choose_path(crate::platform::Pick::Folder) {
                            self.typed.migrate = p;
                            // Where the move would land walks the disk, so it is decided once, when the
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
                    migrate = key::key(ui, t(Key::DoMigrate), Role::Secondary, seat_ok && picked.as_ref().map(|p| Self::landing_ok(&p.at.display().to_string())).unwrap_or(false)).clicked();
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
    }

    /// How many ledger entries and held grants came after the last whole-machine backup (from the last
    /// measurement and the machine settings; `None` when never backed up or not measured).
    pub(super) fn backup_behind(&self) -> Option<u64> {
        crate::machine::backup_behind(self.shell.machine.backup.as_ref(), self.shell.items_now)
    }

    /// The backup point's mark and sentence (the setup check and the wizard read the same judgment,
    /// `firstrun::backup_point`).
    pub(super) fn backup_point(&self) -> (Mark, String) {
        let last = self.shell.machine.backup.as_ref();
        match (crate::firstrun::backup_point(last, self.shell.items_now), last) {
            (crate::firstrun::Shade::Amber, _) => (Mark::Warn, fill1(Key::GapBackupBehind, &self.backup_behind().unwrap_or(0).to_string())),
            (crate::firstrun::Shade::Green, Some(b)) => (Mark::Ok, crate::when::when(b.at)),
            (crate::firstrun::Shade::Grey, Some(b)) => (Mark::Todo, crate::when::when(b.at)),
            _ => (Mark::Warn, t(Key::GapBackupNever).to_string()),
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
            // Off on an empty seat, by the same decision as the identity page.
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
                    // Each face: file, index, where it came from (the two embedded ones with their licence).
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
        if self_check {
            self.act(Action::SelfCheck, now);
        }
    }

    /// The setup check for this seat: each point's mark, name and what to do (real state only).
    pub(super) fn checklist(&self) -> Vec<(Mark, &'static str, String)> {
        let key_mark = if self.shell.anchor.is_some() { Mark::Ok } else { Mark::Bad };
        let pin_mark = if matches!(self.shell.vault, crate::keybox::State::Absent) { Mark::Bad } else { Mark::Ok };
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
        match &self.shell.chain {
            Some(Done::Chain { gas_wei: Some(w), .. }) if *w > 0 => (Mark::Ok, t(Key::Done)),
            Some(Done::Chain { gas_wei: Some(_), .. }) => (Mark::Bad, t(Key::GuideGas)),
            _ => (Mark::Todo, t(Key::GuideGas)),
        }
    }
}
