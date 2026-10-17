//! This product's pages in the window shell: the table the shell asks (`zikaron_ui::nav::Pages`), what each
//! route draws and titles, the toolbar keys of each view's own page, which old page a route stands for, and
//! the places the shell's own keys go (settings, identity keys, record verification).
//!
//! The shell (`chrome`) names no page; every page name of this product is read here or in `crate::nav`.

use super::*;
use crate::nav::{tab as T, View};
use zikaron_ui::nav::Pages;

/// The shell's state over this product's table.
pub(super) type Frame = zikaron_ui::nav::Frame<Place, Route, Stack>;

impl Pages for Win {
    type Place = Place;
    type Route = Route;
    type Stack = Stack;
    type Word = Key;

    fn frame(&self) -> &Frame {
        &self.ux.nav
    }

    fn frame_mut(&mut self) -> &mut Frame {
        &mut self.ux.nav
    }

    fn word(&self, w: Key) -> String {
        t(w).to_string()
    }

    fn rail(&self) -> &'static [crate::nav::Group] {
        crate::nav::rail(self.shell.settings.role)
    }

    fn home(&self) -> Place {
        Place::Home
    }

    fn stack_of(&self, place: Place) -> Stack {
        crate::nav::stack_of(place)
    }

    /// A view's own page is where its rail item goes.
    fn root_of(&self, stack: Stack) -> Place {
        match stack {
            Stack::Home => Place::Home,
            Stack::Settings => Place::SettingsHome,
            Stack::View(v) => crate::nav::rail(self.shell.settings.role)
                .iter()
                .flat_map(|g| g.items.iter())
                .find_map(|i| match i.place {
                    Place::View(x, t) if x == v => Some(Place::View(x, t)),
                    _ => None,
                })
                .unwrap_or(Place::View(v, 0)),
        }
    }

    fn history_of(&self, place: Place) -> (Stack, History) {
        crate::nav::history_of(place, self.shell.settings.role)
    }

    fn root(&self, place: Place) -> Route {
        Route::Root(place)
    }

    fn root_place(&self, route: &Route) -> Option<Place> {
        match route {
            Route::Root(p) => Some(*p),
            _ => None,
        }
    }

    /// Settings pages light the rail's bottom "settings" key, not an item.
    fn lit(&self) -> Option<Place> {
        if self.ux.nav.stack == Stack::Settings {
            return None;
        }
        crate::nav::lit(self.ux.nav.place.unwrap_or(Place::Home), self.shell.settings.role)
    }

    fn same_item(&self, lit: Place, item: Place) -> bool {
        same_item(lit, item)
    }

    /// The alerts item counts what is waiting there.
    fn count(&self, item: Place) -> usize {
        if matches!(item, Place::View(View::Alerts, _)) {
            self.alert_count()
        } else {
            0
        }
    }

    fn name(&self, route: &Route) -> String {
        route.name()
    }

    fn detail(&self, route: &Route) -> bool {
        matches!(route, Route::Work(_) | Route::Pending(_) | Route::Grant(_) | Route::Entry(_) | Route::Other(_) | Route::Held(_))
    }

    fn title(&self, route: &Route) -> String {
        self.title_of(route)
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, route: &Route, now: f64) {
        self.toolbar_acts(ui, route, now);
    }

    fn body(&mut self, ui: &mut egui::Ui, route: &Route, now: f64) {
        self.banner(ui, now);
        self.page_body(ui, route, now);
    }

    /// Drop what belonged to the previous page, sync the shell's page (tests and the trace channel read it),
    /// and have a detail page read its entry's bytes once, off the frame (the action layer reads the disk).
    fn entered(&mut self, now: f64) {
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
                    self.auto(Action::OpenEntry { id: id.clone() }, now);
                }
            }
            _ => {}
        }
    }
}

impl Win {
    /// Land on a place with a fresh history (links on pages, gaps, tests): its view becomes current and shows
    /// that place, with the view's own page behind it when the place is a pushed page. An old page first opens
    /// what it was folded into.
    pub(super) fn go(&mut self, place: Place, now: f64) {
        if let Place::Page(p) = place {
            self.arrive(p);
        }
        zikaron_ui::nav::go(self, place, now);
    }

    /// Where the window stands before anything put it: the shell's page.
    pub(super) fn start_place(&self) -> Place {
        Place::Page(self.shell.page)
    }

    /// Where the rail's "settings" key (and ⌘,) goes.
    pub(super) fn settings_place(&self) -> Place {
        Place::SettingsHome
    }

    /// Where the identity menu's "identity keys" goes.
    pub(super) fn keys_place(&self) -> Place {
        Place::Settings(Section::Keys)
    }

    /// Where a file dropped on the window goes for the user's seat: record verification.
    pub(super) fn record_check_place(&self) -> Place {
        Place::View(View::Verify, T::VERIFY_WORK)
    }

    /// After switching seat, a view that seat lacks lands on home.
    pub(super) fn settle_seat(&mut self) {
        if let Some(p) = self.ux.nav.place {
            let settled = crate::nav::settle(p, self.shell.settings.role);
            if settled != p && matches!(settled, Place::Home) {
                self.ux.nav.hist.clear();
                self.ux.nav.stack = Stack::Home;
                self.ux.nav.place = Some(Place::Home);
            }
        }
    }

    /// The shell's page is followed: a change to it made from outside the window (`Action::Show` from a link, a
    /// test, a command-line request) is gone to the next frame; the window's own moves record the page they set
    /// (`entered`), so this never answers them. The first frame only takes note: the window starts where it was
    /// put (from the shell's page itself when nothing else put it).
    pub(super) fn follow_page(&mut self, now: f64) {
        let page = self.shell.page;
        match self.ux.page_seen {
            None => self.ux.page_seen = Some(page),
            Some(seen) if seen != page => {
                self.ux.page_seen = Some(page);
                let here = route_page(&self.route(), self.shell.settings.role);
                if here != Some(page) {
                    self.go(Place::Page(page), now);
                }
            }
            Some(_) => {}
        }
    }

    /// At start, with the gate down: the wizard opens by itself at its first open step when a test driver
    /// asked for it or the machine wants it.
    pub(super) fn wizard_by_itself(&mut self) {
        let pr = self.progress();
        if self.ux.wizard_forced || pr.wants_wizard(self.shell.settings.role) {
            self.wizard_open(pr.first_open().unwrap_or(crate::nav::Step::Pin));
        }
    }

    /// The intent of pages folded into a parent item. Exhaustive: each new `Page` makes the compiler ask
    /// whether it has an intent here.
    pub(super) fn arrive(&mut self, p: Page) {
        match p {
            Page::Audit => self.ux.u3.audit_open = true,
            Page::Adopt => self.u3_form_open(U3Form::Adopt),
            Page::Succeed => self.u3_form_open(U3Form::Succeed),
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
            | Page::Depth
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
            | Page::About => {}
        }
    }

    /// The page for a route.
    fn page_body(&mut self, ui: &mut egui::Ui, route: &Route, now: f64) {
        match route {
            Route::Root(Place::Home) => self.page_home(ui, now),
            Route::Root(Place::View(View::Works, tab)) => {
                if *tab == T::WORKS_PENDING {
                    self.works_pending(ui, now)
                } else {
                    self.works(ui, now)
                }
            }
            Route::Root(Place::View(View::Grants, _)) => self.grants_list(ui, now),
            Route::Root(Place::View(View::Verify, tab)) => self.verify_page(ui, *tab, now),
            Route::Root(Place::View(View::Log, _)) => self.ledger_page(ui, now),
            Route::Root(Place::View(View::Alerts, _)) => self.alerts_page(ui, now),
            Route::Root(Place::View(View::MyGrants, _)) => self.vault_page(ui, now),
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

    /// The toolbar title of a route.
    pub(super) fn title_of(&self, route: &Route) -> String {
        let rows: &[crate::ledgerx::Row] = self.shell.rows.as_ref().map(|(r, _)| r.as_slice()).unwrap_or(&[]);
        let entry_title = |id: &str| rows.iter().find(|r| r.id.eq_ignore_ascii_case(id)).map(|r| format!("#{} \u{b7} {}", r.seq, t(user_kind_key(r.kind))));
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
            })
            .to_string(),
            Route::Section(s) => t(s.key()).to_string(),
            Route::Work(id) => rows.iter().find(|r| r.id.eq_ignore_ascii_case(id)).map(human_summary).unwrap_or_default(),
            Route::Pending(id) | Route::Entry(id) => entry_title(id).unwrap_or_default(),
            Route::Grant(id) => rows.iter().find(|r| r.id.eq_ignore_ascii_case(id)).map(|r| format!("#{} \u{b7} {}", r.seq, t(Key::KindGrant))).unwrap_or_default(),
            Route::Other(seq) => self.other_title(*seq),
            Route::Held(id) => self.held_title(id),
            Route::Kit => t(Key::U3UseForKit).to_string(),
            Route::NewGrant => t(Key::V2NewGrant).to_string(),
            Route::Relicense => t(Key::U4RelicenseTitle).to_string(),
        }
    }

    /// The buttons at the right of a view's own page's toolbar.
    fn toolbar_acts(&mut self, ui: &mut egui::Ui, route: &Route, now: f64) {
        let Route::Root(place) = route else { return };
        if matches!(self.shell.unfetched, Some(crate::restorex::State::NewerElsewhere { .. })) {
            mark::pill(ui, t(Key::NoteReader), PillTone::Warn);
        }
        match place {
            Place::View(View::Works, tab) => {
                self.ensure_rows(now);
                let cells = [seg::Cell::from(t(Key::U3FilterAll)), seg::Cell { text: t(Key::ItemUnanchored), count: Some(self.shell.queue.len()) }];
                if let Some(i) = seg::seg(ui, "works-tabs", &cells, usize::from(*tab == T::WORKS_PENDING)) {
                    self.set_tab(Place::View(View::Works, if i == 1 { T::WORKS_PENDING } else { T::WORKS_ALL }), now);
                }
            }
            Place::View(View::Grants, _) => {
                self.ensure_grants(now);
                let (_s, r) = page::Page::new().primary(ui, t(Key::V2NewGrant));
                if r.clicked() {
                    self.grant_form_fresh();
                    self.push(Route::NewGrant, now);
                }
            }
            Place::View(View::Verify, tab) => {
                let role = self.shell.settings.role;
                let list = crate::nav::tabs(View::Verify, role);
                let labels: Vec<seg::Cell> = list.iter().map(|x| seg::Cell::from(t(tab_key(View::Verify, *x)))).collect();
                let cur = list.iter().position(|x| x == tab).unwrap_or(0);
                if let Some(i) = seg::seg(ui, "verify-tabs", &labels, cur) {
                    self.set_tab(Place::View(View::Verify, list[i]), now);
                }
            }
            Place::View(View::Log, _) => self.ledger_acts(ui, now),
            Place::View(View::MyGrants, _) => self.vault_acts(ui, now),
            _ => {}
        }
    }
}

/// Which old page a route stands for (the shell records it: tests and the trace channel read it).
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

/// Whether a rail item is the lit one (views ignore tabs).
pub(super) fn same_item(lit: Place, item: Place) -> bool {
    match (lit, item) {
        (Place::View(a, _), Place::View(b, _)) => a == b,
        (a, b) => a == b,
    }
}

/// The page shell records for a view's tab (`shell.page`: tests and the trace channel read it).
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
    }
}

/// The text of a verify tab.
pub(super) fn tab_key(v: View, tab: u8) -> Key {
    match (v, tab) {
        (View::Verify, T::VERIFY_WORK) => Key::V2TabVerifyWork,
        (View::Verify, T::VERIFY_OTHERS) => Key::V2TabOthers,
        (View::Verify, _) => Key::V2TabCheckGrant,
        (_, _) => v.key(),
    }
}

pub(super) fn step_name(s: crate::wizard::Step) -> Key {
    match s {
        crate::wizard::Step::Terms => Key::StepTerms,
        crate::wizard::Step::Anchor => Key::StepAnchor,
    }
}

/// Where a watch row's "next" leads: an unanswered node goes to network settings; expiring holdings and
/// missing chains go to my grants.
pub(super) fn gap_target(g: crate::watchx::Gap) -> Place {
    use crate::watchx::Say;
    match g.say {
        Say::AuditUnavailable => Place::Settings(Section::Network),
        Say::HoldingNoNow | Say::HoldingExpired | Say::HoldingSoon => Place::Page(Page::Vault),
        _ => Place::Page(g.go),
    }
}

/// The settled place for [`gap_target`] (the link is named after it).
pub(super) fn gap_place(g: crate::watchx::Gap, role: crate::roles::Role) -> Place {
    match gap_target(g) {
        Place::Page(p) => crate::nav::home_of(p, role),
        other => other,
    }
}
