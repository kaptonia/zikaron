use super::*;

pub(super) fn task_key(k: crate::task::Kind) -> Key {
    use crate::task::Kind;
    match k {
        Kind::SelfCheck => Key::TaskSelfCheck,
        Kind::Archive => Key::TaskArchive,
        Kind::Chain => Key::TaskChain,
        Kind::ChainTime => Key::TaskChainTime,
        Kind::Rotate => Key::TaskRotate,
        Kind::Reconcile => Key::TaskReconcile,
        Kind::Audit => Key::TaskAudit,
        Kind::Ledger => Key::TaskLedger,
        Kind::Anchor => Key::TaskAnchor,
        Kind::Kit => Key::TaskKit,
        Kind::Grants => Key::TaskGrants,
        Kind::Book => Key::TaskBook,
        Kind::Diligence => Key::TaskDiligence,
        Kind::Verify => Key::TaskVerify,
        Kind::Delivery => Key::TaskDelivery,
        Kind::Review => Key::TaskReview,
        Kind::Held => Key::TaskHeld,
        Kind::Badge => Key::TaskBadge,
        Kind::Check => Key::TaskCheck,
        Kind::Keystore => Key::TaskKeystore,
        Kind::Vault => Key::TaskVault,
        Kind::Publish => Key::DoCheckPublished,
        Kind::Fetch => Key::DoFetchLedger,
        Kind::Vet => Key::TaskVet,
        Kind::Peek => Key::PeekReading,
        Kind::Gate => Key::TaskGate,
        Kind::Backup => Key::WizBackupTitle,
        Kind::ReadNet => Key::TaskReadNet,
        Kind::Basis => Key::TaskBasis,
        Kind::Gas => Key::U3GasEstimate,
        Kind::Take => Key::TaskTake,
        Kind::Record => Key::TaskRecord,
        Kind::Migrate => Key::TaskMigrate,
        Kind::Path => Key::TaskPath,
        Kind::CliPath => Key::TaskCliPath,
        Kind::Seat => Key::TaskSeat,
        Kind::Extract => Key::DoExtract,
        Kind::CleanList => Key::TaskCleanList,
        Kind::Clean => Key::CleanTitle,
        Kind::Persona => Key::TaskPersona,
        Kind::FaceBooks => Key::TaskFaceBooks,
        Kind::RerootFee => Key::TaskRerootFee,
        Kind::FaceJoined => Key::TaskFaceJoined,
        Kind::Behind => Key::TaskBehind,
        Kind::Look => Key::TaskLook,
        Kind::Regain => Key::TaskRegain,
        Kind::Keep => Key::TaskKeep,
    }
}

pub(super) fn task_long(k: crate::task::Kind) -> bool {
    use crate::task::Kind;
    !matches!(k, Kind::Ledger | Kind::Grants | Kind::Held | Kind::Vet | Kind::Peek | Kind::Vault | Kind::Look | Kind::Regain | Kind::Keep)
}

impl Win {
    pub(super) fn own_turn(&mut self, ctx: &egui::Context, now: f64) {
        let _ = (ctx, now);
    }

    pub(super) fn own_rang(&mut self) -> Vec<String> {
        std::mem::take(&mut self.shell.rung)
            .into_iter()
            .map(|a| match a.kind {
                crate::sentinelx::Kind::Revoked => fill1(Key::SaidAlarm, t(Key::AlarmRevoked)),
                crate::sentinelx::Kind::Handed => fill1(Key::SaidAlarm, t(Key::AlarmHandedPlain)),
            })
            .collect()
    }

    pub(super) fn own_card_more(&self, seat: crate::roles::Role) -> Option<String> {
        let _ = seat;
        None
    }

    pub(super) fn own_tag(&self, row: &crate::identity::Row) -> Option<&'static str> {
        let _ = row;
        None
    }

    pub(super) fn own_recount(&mut self) -> bool {
        crate::action::recount_audit(&mut self.shell)
    }
}

pub(super) const APP_ID: &str = "zikaron-desk";

pub(super) fn icon() -> egui::IconData {
    egui::IconData { rgba: crate::about::ICON.to_vec(), width: 256, height: 256 }
}

pub(super) const MENU_BAR: bool = true;

pub(super) const GATE_KEY: Option<Key> = None;

pub(super) fn own_start(start: &Start, ux: &mut Ux, role: crate::roles::Role) {
    if let Start::Scene(scene) = start {
        if let Scene::Page(p) | Scene::Sheet(p, _) | Scene::Data(p, _) = scene {
            ux.nav.land(crate::nav::history_of(*p, role));
        }
        ux.wizard_asked = true;
    }
}

pub(super) fn own_stage(win: &mut Win, start: &Start) {
    if let Start::Scene(scene) = start {
        super::probes::stage_scene(win, *scene);
    }
}

impl Win {
    pub(super) fn own_net_task(&self) -> crate::task::Kind {
        crate::task::Kind::Chain
    }

    pub(super) fn own_net_go(&mut self, now: f64) {
        self.act(Action::ReadChain, now);
    }

    pub(super) fn own_after_rest(&mut self, ui: &mut egui::Ui, now: f64) {
        self.set_chain_group(ui, now);
        stagger(ui, 5, |ui| self.set_read_nets(ui));
        stagger(ui, 6, |ui| self.set_publish(ui));
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum OwnBk {
    Mirror,
}

impl Win {
    pub(super) fn own_bk_sheet(&mut self, ctx: &egui::Context, own: OwnBk, trouble: Option<crate::fault::Fault>, now: f64) -> (Option<Action>, bool, bool) {
        match own {
            OwnBk::Mirror => self.mirror_sheet(ctx, trouble, now),
        }
    }

    pub(super) fn own_bk_done(&self, r: &Applied) -> bool {
        matches!(r, Applied::Mirrored { .. })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum OwnIdPress {
    Rotate,
    Consent,
}

pub(super) fn own_delete_effect(seat: crate::roles::Role) -> Key {
    match seat {
        crate::roles::Role::Author => Key::IdDeleteAuthorEffect,
        crate::roles::Role::Grantee => Key::IdDeleteGranteeEffect,
    }
}

impl Win {
    pub(super) fn own_id_rows(&self, ui: &mut egui::Ui, f: &mut card::Form, current: bool, unseated: bool, can_sign: bool) -> Option<OwnIdPress> {
        let rotate = f.go(ui, t(Key::RotTitle), "", current && !unseated, "").clicked();
        let r = f.go(ui, t(Key::SetConsentRow), "", current && !unseated && can_sign, "");
        zikaron_ui::probe::spot(ui.ctx(), "persona:consentRow", r.rect);
        if rotate {
            Some(OwnIdPress::Rotate)
        } else if r.clicked() {
            Some(OwnIdPress::Consent)
        } else {
            None
        }
    }

    pub(super) fn own_id_pressed(&mut self, p: OwnIdPress, now: f64) {
        match p {
            OwnIdPress::Rotate => {
                self.ux.rotate_put_away = false;
                self.ux.rot_walked = false;
                self.rotate_sheet_open(now);
            }
            OwnIdPress::Consent => self.consent_open(),
        }
    }

    pub(super) fn own_key_rows(&self, row: Option<&crate::identity::Row>, seat: crate::roles::Role) -> Vec<(&'static str, Val)> {
        let mut raw = Vec::new();
        if let Some(o) = row.and_then(|r| r.office(seat)) {
            raw.push((t(Key::IdGeneration), Val::figure(o.generation.to_string())));
        }
        if let Some(p) = self.shell.anchor_public {
            raw.push((t(Key::IdPublicKey), Val::mono(zikaron::hexfmt::encode(&p))));
        }
        raw
    }

    pub(super) fn own_details(&self, seated: bool, seat: crate::roles::Role) -> Vec<(&'static str, Val)> {
        vec![
            (t(Key::SignFaces), Val::mono(crate::sign::Face::ALL.iter().map(|f| f.as_str()).collect::<Vec<_>>().join(" "))),
            (
                t(Key::SignSeatDomains),
                if seated {
                    Val::mono(crate::sign::Use::all().into_iter().filter(|u| crate::sign::seat_may(seat, *u)).map(|u| u.as_str()).collect::<Vec<_>>().join(" "))
                } else {
                    Val::text(t(Key::IdSeatEmpty))
                },
            ),
        ]
    }

    pub(super) fn own_hold(&self, a: &Action) -> Option<crate::fault::Fault> {
        let _ = a;
        None
    }
}

impl Win {
    pub(super) fn id_network_now(&self) -> String {
        if self.ux.wizard.is_some() {
            return self.wiz_network_pick();
        }
        self.ux.id_network.clone().unwrap_or_else(|| crate::machine::pick(&self.shell.machine))
    }

    pub(super) fn id_import_network_now(&self) -> String {
        if self.ux.wizard.is_some() && !crate::own::OwnProgress::of(&self.shell).network {
            return crate::deploy::UNDECIDED.to_string();
        }
        self.id_network_now()
    }

    pub(super) fn own_network_row(&mut self, f: &mut card::Form, ui: &mut egui::Ui, salt: &str) {
        if self.ux.wizard.is_some() {
            return;
        }
        let now = self.id_network_now();
        let names = crate::deploy::choices();
        let labels: Vec<String> = names.iter().map(|n| network_label(n)).collect();
        let at = names.iter().position(|n| *n == now).unwrap_or(0);
        let items: Vec<menu::Item> = labels.iter().enumerate().map(|(i, l)| menu::Item::Row(menu::Row { label: l, check: Some(i == at), ..Default::default() })).collect();
        let mut picked = None;
        f.row_with(ui, None, t(Key::SetChain), "", card::Value::None, |ui| picked = menu::menu_key(ui, salt, &labels[at], false, 160.0, &items));
        if let Some(i) = picked {
            self.ux.id_network = names.get(i).map(|n| n.to_string());
        }
    }
}

pub(super) fn own_net_key(ui: &mut egui::Ui, bare: bool, phase: Phase) -> bool {
    key::show(ui, key::Key::new(t(Key::DoReadChain), Role::Secondary).enabled(!bare).phase(phase)).clicked()
}

pub(super) const IMPORT_TABS: &[super::identity::ImportTab] = &[super::identity::ImportTab::Words, super::identity::ImportTab::Key, super::identity::ImportTab::File];

pub(super) const IMPORT_SEAT_ROW: bool = true;

impl Win {
    pub(super) fn own_gas_fact(&self) -> Option<(Mark, zikaron_ui::layer::Amount)> {
        None
    }

    pub(super) fn own_primary_how(&self, primary: Option<&crate::identity::Row>) -> Option<(String, String)> {
        let _ = primary;
        None
    }

    pub(super) fn own_keys_after(&mut self, ui: &mut egui::Ui, now: f64) {
        let _ = (ui, now);
    }
}

impl Win {
    pub(super) fn own_backup_rows(&self, ui: &mut egui::Ui, f: &mut card::Form) -> Option<OwnBk> {
        let author = self.shell.settings.role == crate::roles::Role::Author;
        (author && f.go(ui, t(Key::MirrorTitle), "", true, "").clicked()).then_some(OwnBk::Mirror)
    }

    pub(super) fn own_backup_after(&mut self, ui: &mut egui::Ui, now: f64) {
        let _ = (ui, now);
    }
}

impl Win {
    pub(super) fn own_rail_extra(&self, ui: &mut egui::Ui) -> bool {
        let _ = ui;
        false
    }

    pub(super) fn own_rail_go(&mut self, now: f64) {
        let _ = now;
    }

    pub(super) fn own_finished_words(&self) -> Option<String> {
        None
    }

    pub(super) fn own_wiz_enter(&mut self, step: crate::nav::Step) {
        let _ = step;
    }
}

impl Win {
    pub(super) fn own_pushed(&self, place: Place, hist: &mut History) {
        let _ = (place, hist);
    }
}

impl Win {
    pub(super) fn own_net_overview(&mut self, ui: &mut egui::Ui, now: f64) -> bool {
        let _ = (ui, now);
        false
    }

    pub(super) fn own_net_details(&self) -> Option<Vec<(&'static str, Val)>> {
        None
    }

    pub(super) fn own_home_chain(&self) -> Option<u64> {
        self.shell.settings.chain_id
    }
}
