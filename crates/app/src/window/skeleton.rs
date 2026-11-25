use super::*;
use zikaron_ui::nav::Pages;

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
        let (stack, mut hist) = crate::nav::history_of(place, self.shell.settings.role);
        self.own_pushed(place, &mut hist);
        (stack, hist)
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

    fn lit(&self) -> Option<Place> {
        if self.ux.nav.stack == Stack::Settings {
            return None;
        }
        crate::nav::lit(self.ux.nav.place.unwrap_or(Place::Home), self.shell.settings.role)
    }

    fn same_item(&self, lit: Place, item: Place) -> bool {
        same_item(lit, item)
    }

    fn count(&self, item: Place) -> usize {
        self.rail_count(item)
    }

    fn name(&self, route: &Route) -> String {
        route.name()
    }

    fn detail(&self, route: &Route) -> bool {
        self.detail_route(route)
    }

    fn title(&self, route: &Route) -> String {
        self.title_of(route)
    }

    fn title_in_bar(&self, route: &Route) -> bool {
        self.bar_title(route)
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, route: &Route, now: f64) {
        self.toolbar_acts(ui, route, now);
    }

    fn body(&mut self, ui: &mut egui::Ui, route: &Route, now: f64) {
        self.banner(ui, now);
        self.body_before(ui, now);
        self.page_body(ui, route, now);
    }

    fn entered(&mut self, now: f64) {
        self.page_entered(now);
    }
}

impl Win {
    pub(super) fn go(&mut self, place: Place, now: f64) {
        if let Place::Page(p) = place {
            self.arrive(p);
        }
        zikaron_ui::nav::go(self, place, now);
    }

    pub(super) fn start_place(&self) -> Place {
        Place::Page(self.shell.page)
    }

    pub(super) fn settings_place(&self) -> Place {
        Place::SettingsHome
    }

    pub(super) fn keys_place(&self) -> Place {
        Place::Settings(Section::Keys)
    }

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
        self.follow_extra(now);
    }

    pub(super) fn wizard_by_itself(&mut self) {
        let pr = self.progress();
        if self.ux.wizard_forced || pr.wants_wizard(self.shell.settings.role) {
            self.wizard_open(pr.first_open().unwrap_or(crate::nav::Step::Pin));
        }
    }
}

pub(super) fn same_item(lit: Place, item: Place) -> bool {
    match (lit, item) {
        (Place::View(a, _), Place::View(b, _)) => a == b,
        (a, b) => a == b,
    }
}

