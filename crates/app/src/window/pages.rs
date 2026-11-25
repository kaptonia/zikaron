use super::*;
use crate::nav::{tab as T, View};

impl Win {
    pub(super) fn rail_count(&self, item: Place) -> usize {
        if matches!(item, Place::View(View::Alerts, _)) {
            self.alert_count()
        } else {
            0
        }
    }

    pub(super) fn detail_route(&self, route: &Route) -> bool {
        matches!(route, Route::Work(_) | Route::Pending(_) | Route::Grant(_) | Route::Entry(_) | Route::Other(_) | Route::Held(_))
    }

    pub(super) fn bar_title(&self, route: &Route) -> bool {
        matches!(route, Route::Root(Place::Settings(_)) | Route::Section(_))
    }

    pub(super) fn page_entered(&mut self, now: f64) {
        self.ux.u3.drop_armed = None;
        let route = self.route();
        let page = route_page(&route, self.shell.settings.role);
        if let Some(p) = page {
            if self.shell.page != p {
                self.auto(Action::Show(p), now);
            }
            self.ux.page_seen = Some(p);
        }
        match &route {
            Route::Work(id) | Route::Entry(id) | Route::Grant(id) | Route::Pending(id) => {
                if self.opened.as_ref().map(|d| !d.id.eq_ignore_ascii_case(id)).unwrap_or(true) {
                    self.open_entry(id);
                }
            }
            _ => {}
        }
    }
    pub(super) fn body_before(&mut self, _ui: &mut egui::Ui, _now: f64) {}

    pub(super) fn follow_extra(&mut self, _now: f64) {}

    pub(super) fn record_check_place(&self) -> Place {
        Place::View(View::Verify, T::VERIFY_WORK)
    }

    pub(super) fn arrive(&mut self, p: Page) {
        match p {
            Page::Audit => self.ux.u3.audit_open = true,
            Page::Upstreams => self.ux.u4.vault_by_issuer = true,
            Page::FirstRun => {
                let step = self.progress().first_open().unwrap_or(crate::nav::Step::Pin);
                self.wizard_open(step);
            }
            Page::Ledger
            | Page::Anchoring
            | Page::Queue
            | Page::Grant
            | Page::Grants
            | Page::Revoke
            | Page::FirstWindow
            | Page::Kit
            | Page::Reader
            | Page::Check
            | Page::Watch
            | Page::Diligence
            | Page::Verifier
            | Page::Delivery
            | Page::Vault
            | Page::Sentinel
            | Page::Relicense
            | Page::Badge
            | Page::Identity
            | Page::Archive
            | Page::Mirror
            | Page::Skeleton
            | Page::About
            | Page::Persona => {}
        }
    }

    pub(super) fn page_body(&mut self, ui: &mut egui::Ui, route: &Route, now: f64) {
        match route {
            Route::Root(Place::Home) => self.page_home(ui, now),
            Route::Root(Place::View(View::Works, tab)) => tab_body(ui, "works", u64::from(*tab), |ui| {
                if *tab == T::WORKS_PENDING {
                    self.works_pending(ui, now)
                } else {
                    self.works(ui, now)
                }
            }),
            Route::Root(Place::View(View::Grants, _)) => self.grants_list(ui, now),
            Route::Root(Place::View(View::Verify, tab)) => self.verify_page(ui, *tab, now),
            Route::Root(Place::View(View::Log, _)) => self.ledger_page(ui, now),
            Route::Root(Place::View(View::Alerts, _)) => self.alerts_page(ui, now),
            Route::Root(Place::View(View::MyGrants, _)) => self.vault_page(ui, now),
            Route::Root(Place::View(View::Persona, tab)) => self.persona_page(ui, *tab, now),
            Route::Root(Place::SettingsHome) | Route::Root(Place::Settings(_)) => self.settings_home(ui, now),
            Route::Section(s) => self.settings_section(ui, *s, now),
            Route::Root(Place::Page(_)) => self.page_home(ui, now),
            Route::Work(id) => self.work_detail(ui, id, now),
            Route::Pending(id) => self.pending_detail(ui, id, now),
            Route::Kit => self.kit_page(ui, now),
            Route::NewGrant => self.grant_form_page(ui, now),
            Route::Grant(id) | Route::Entry(id) => self.entry_detail(ui, id, now),
            Route::Other(seq) => self.other_entry(ui, *seq, now),
            Route::Held(id) => self.held_detail(ui, id, now),
            Route::Relicense => self.relicense_page(ui, now),
        }
    }

    pub(super) fn title_of(&self, route: &Route) -> String {
        let chain_now = self.shell.chain_now();
        let rows: &[crate::ledgerx::Row] = self.shell.rows.as_ref().map(|(r, _)| r.as_slice()).unwrap_or(&[]);
        let entry_title = |id: &str| rows.iter().find(|r| r.id.eq_ignore_ascii_case(id)).map(|r| format!("#{} \u{b7} {}", r.seq, t(row_kind_key(r))));
        match route {
            Route::Root(Place::Home) | Route::Root(Place::Page(_)) => t(Key::NavHome).to_string(),
            Route::Root(Place::SettingsHome) | Route::Root(Place::Settings(_)) => t(Key::NavSettings).to_string(),
            Route::Root(Place::View(v, _)) => t(match v {
                View::Works => Key::NavWorksView,
                View::Grants => Key::KindGrant,
                View::Verify => Key::NavVerifyView,
                View::Log => Key::NavLogView,
                View::Alerts => Key::NavAlertsView,
                View::MyGrants => Key::NavMyGrantsView,
                View::Persona => Key::PagePersona,
            })
            .to_string(),
            Route::Section(s) => t(s.key()).to_string(),
            Route::Work(id) => rows.iter().find(|r| r.id.eq_ignore_ascii_case(id)).map(|x| human_summary(x, chain_now)).unwrap_or_default(),
            Route::Pending(id) | Route::Entry(id) => entry_title(id).unwrap_or_default(),
            Route::Grant(id) => rows.iter().find(|r| r.id.eq_ignore_ascii_case(id)).map(|r| format!("#{} \u{b7} {}", r.seq, t(Key::KindGrant))).unwrap_or_default(),
            Route::Other(seq) => self.other_title(*seq),
            Route::Held(id) => self.held_title(id),
            Route::Kit => t(Key::U3UseForKit).to_string(),
            Route::NewGrant => t(Key::V2NewGrant).to_string(),
            Route::Relicense => t(Key::U4RelicenseTitle).to_string(),
        }
    }

    pub(super) fn toolbar_acts(&mut self, ui: &mut egui::Ui, route: &Route, now: f64) {
        let Route::Root(place) = route else { return };
        if matches!(self.shell.unfetched, Some(crate::restorex::State::NewerElsewhere { .. })) {
            mark::pill(ui, t(Key::NoteReader), PillTone::Warn);
        }
        match place {
            Place::View(View::Works, tab) => {
                self.ensure_rows(now);
                let cells = [seg::Cell::from(t(Key::U3FilterAll)), seg::Cell { text: t(Key::ItemUnanchored), count: Some(self.shell.queue.len()) }];
                if let Some(i) = seg::seg(ui, "works-tabs", &cells, usize::from(*tab == T::WORKS_PENDING)) {
                    self.seg_tab(Place::View(View::Works, if i == 1 { T::WORKS_PENDING } else { T::WORKS_ALL }), now);
                }
            }
            Place::View(View::Grants, _) => {
                self.ensure_grants(now);
                let (_s, r) = page::Page::new().primary_lead(ui, t(Key::V2NewGrant), zikaron_ui::icons::Glyph::Plus);
                if r.clicked() {
                    self.grant_form_fresh();
                    self.push(Route::NewGrant, now);
                }
            }
            Place::View(View::Log, _) => self.ledger_acts(ui, now),
            Place::View(View::MyGrants, _) => self.vault_acts(ui, now),
            Place::View(View::Persona, tab) => self.persona_acts(ui, *tab, now),
            _ => {}
        }
    }
}

pub(super) fn route_page(route: &Route, role: crate::roles::Role) -> Option<Page> {
    Some(match route {
        Route::Kit => Page::Kit,
        Route::NewGrant => Page::Grant,
        Route::Relicense => Page::Relicense,
        Route::Work(_) => Page::Anchoring,
        Route::Pending(_) => Page::Queue,
        Route::Grant(_) => Page::Grants,
        Route::Entry(_) => Page::Ledger,
        Route::Other(_) => match role {
            crate::roles::Role::Author => Page::Reader,
            crate::roles::Role::Grantee => Page::Diligence,
        },
        Route::Held(_) => Page::Vault,
        Route::Section(_) | Route::Root(Place::SettingsHome) | Route::Root(Place::Settings(_)) | Route::Root(Place::Home) | Route::Root(Place::Page(_)) => return None,
        Route::Root(Place::View(v, tab)) => view_page(*v, *tab, role),
    })
}

pub(super) fn view_page(v: View, tab: u8, role: crate::roles::Role) -> Page {
    match v {
        View::Works => match tab {
            T::WORKS_PENDING => Page::Queue,
            T::WORKS_KIT => Page::Kit,
            _ => Page::Anchoring,
        },
        View::Grants => match tab {
            T::GRANTS_NEW => Page::Grant,
            _ => Page::Grants,
        },
        View::Verify => match tab {
            T::VERIFY_WORK => Page::Verifier,
            T::VERIFY_OTHERS => match role {
                crate::roles::Role::Author => Page::Reader,
                crate::roles::Role::Grantee => Page::Diligence,
            },
            _ => Page::Check,
        },
        View::Log => Page::Ledger,
        View::Alerts => Page::Watch,
        View::MyGrants => match tab {
            T::HELD_RELICENSE => Page::Relicense,
            _ => Page::Vault,
        },
        View::Persona => Page::Persona,
    }
}

pub(super) fn tab_key(v: View, tab: u8) -> Key {
    match (v, tab) {
        (View::Verify, T::VERIFY_WORK) => Key::V2TabVerifyWork,
        (View::Verify, T::VERIFY_OTHERS) => Key::V2TabOthers,
        (View::Verify, _) => Key::V2TabCheckGrant,
        (View::Persona, T::PERSONA_NAMES) => Key::PsTabNames,
        (View::Persona, T::PERSONA_HOME) => Key::PsTabHome,
        (View::Persona, _) => Key::U3Overview,
        (_, _) => v.key(),
    }
}

pub(super) fn gap_target(g: crate::watchx::Gap) -> Place {
    use crate::watchx::Say;
    match g.say {
        Say::AuditUnavailable => Place::Settings(Section::Network),
        Say::HoldingNoNow | Say::HoldingExpired | Say::HoldingSoon => Place::Page(Page::Vault),
        _ => Place::Page(g.go),
    }
}

pub(super) fn gap_place(g: crate::watchx::Gap, role: crate::roles::Role) -> Place {
    match gap_target(g) {
        Place::Page(p) => crate::nav::home_of(p, role),
        other => other,
    }
}
