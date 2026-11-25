use crate::lang::Key;
use crate::roles::Role;
use zikaron_ui::icons::Glyph;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Page {
    #[default]
    FirstRun,
    Identity,
    Archive,
    Mirror,
    Ledger,
    Kit,
    Grant,
    Grants,
    FirstWindow,
    Revoke,
    Reader,
    Audit,
    Diligence,
    Verifier,
    Delivery,
    Vault,
    Sentinel,
    Upstreams,
    Badge,
    Relicense,
    Check,
    Watch,
    Anchoring,
    Queue,
    Skeleton,
    About,
    Persona,
}

impl Page {
    pub const ALL: [Page; 27] = [
        Page::Ledger,
        Page::Anchoring,
        Page::Queue,
        Page::Audit,
        Page::Grant,
        Page::Grants,
        Page::Revoke,
        Page::FirstWindow,
        Page::Kit,
        Page::Reader,
        Page::Check,
        Page::Watch,
        Page::Diligence,
        Page::Verifier,
        Page::Delivery,
        Page::Vault,
        Page::Upstreams,
        Page::Sentinel,
        Page::Relicense,
        Page::Badge,
        Page::FirstRun,
        Page::Identity,
        Page::Archive,
        Page::Mirror,
        Page::Skeleton,
        Page::About,
        Page::Persona,
    ];

    pub fn title(self) -> &'static str {
        crate::lang::t(self.key())
    }

    pub fn key(self) -> crate::lang::Key {
        match self {
            Page::FirstRun => crate::lang::Key::PageFirstRun,
            Page::Identity => crate::lang::Key::PageIdentity,
            Page::Archive => crate::lang::Key::PageArchive,
            Page::Mirror => crate::lang::Key::PageMirror,
            Page::Ledger => crate::lang::Key::PageLedger,
            Page::Kit => crate::lang::Key::PageKit,
            Page::Grant => crate::lang::Key::PageGrant,
            Page::Grants => crate::lang::Key::PageGrants,
            Page::FirstWindow => crate::lang::Key::PageFirstWindow,
            Page::Revoke => crate::lang::Key::PageRevoke,
            Page::Reader => crate::lang::Key::PageReader,
            Page::Audit => crate::lang::Key::PageAudit,
            Page::Diligence => crate::lang::Key::PageDiligence,
            Page::Verifier => crate::lang::Key::PageVerifier,
            Page::Delivery => crate::lang::Key::PageDelivery,
            Page::Vault => crate::lang::Key::PageVault,
            Page::Sentinel => crate::lang::Key::PageSentinel,
            Page::Upstreams => crate::lang::Key::PageUpstreams,
            Page::Badge => crate::lang::Key::PageBadge,
            Page::Relicense => crate::lang::Key::PageRelicense,
            Page::Check => crate::lang::Key::PageCheck,
            Page::Watch => crate::lang::Key::PageWatch,
            Page::Anchoring => crate::lang::Key::PageAnchoring,
            Page::Queue => crate::lang::Key::PageQueue,
            Page::Skeleton => crate::lang::Key::PageSkeleton,
            Page::About => crate::lang::Key::PageAbout,
            Page::Persona => crate::lang::Key::PagePersona,
        }
    }

    pub fn icon(self) -> zikaron_ui::icons::Icon {
        use zikaron_ui::icons::Icon;
        match self {
            Page::Ledger | Page::Anchoring | Page::Queue | Page::Audit => Icon::Ledger,
            Page::Grant | Page::Grants | Page::Revoke | Page::FirstWindow => Icon::Grant,
            Page::Kit | Page::Reader => Icon::Kit,
            Page::Diligence | Page::Verifier | Page::Delivery => Icon::Kit,
            Page::Check => Icon::Kit,
            Page::Watch => Icon::Ledger,
            Page::Vault
            | Page::Sentinel
            | Page::Upstreams
            | Page::Badge
            | Page::Relicense => Icon::Grant,
            Page::FirstRun
            | Page::Identity
            | Page::Archive
            | Page::Mirror
            | Page::Skeleton
            | Page::About => Icon::Gear,
            Page::Persona => Icon::Gear,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Section {
    Language,
    Appearance,
    Keys,
    Network,
    Notify,
    Data,
    Cli,
    About,
}

impl Section {
    pub const ALL: [Section; 8] = [
        Section::Language,
        Section::Appearance,
        Section::Keys,
        Section::Network,
        Section::Notify,
        Section::Data,
        Section::Cli,
        Section::About,
    ];

    pub fn key(self) -> Key {
        match self {
            Section::Language => Key::SetLangTime,
            Section::Appearance => Key::SetAppearance,
            Section::Keys => Key::SetKeys,
            Section::Network => Key::SetChain,
            Section::Notify => Key::SetCadence,
            Section::Data => Key::SetData,
            Section::Cli => Key::SetCliGroup,
            Section::About => Key::PageAbout,
        }
    }

    pub fn glyph(self) -> zikaron_ui::icons::Glyph {
        use zikaron_ui::icons::Glyph;
        match self {
            Section::Language => Glyph::Lang,
            Section::Appearance => Glyph::Sun,
            Section::Keys => Glyph::Key,
            Section::Network => Glyph::Globe,
            Section::Notify => Glyph::Watch,
            Section::Data => Glyph::Data,
            Section::Cli => Glyph::Terminal,
            Section::About => Glyph::Info,
        }
    }

    pub fn note(self, role: Role) -> Key {
        match self {
            Section::Language => Key::SetNoteLang,
            Section::Appearance => Key::SetNoteAppearance,
            Section::Keys => Key::SetNoteKeys,
            Section::Network => Key::SetNoteNetwork,
            Section::Notify => match role {
                Role::Author => Key::SetNoteNotifyAuthor,
                Role::Grantee => Key::SetNoteNotifyGrantee,
            },
            Section::Data => Key::SetNoteData,
            Section::Cli => Key::SetNoteCli,
            Section::About => Key::SetNoteAbout,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Section::Language => "language",
            Section::Appearance => "appearance",
            Section::Keys => "keys",
            Section::Network => "network",
            Section::Notify => "notify",
            Section::Data => "data",
            Section::Cli => "cli",
            Section::About => "about",
        }
    }
}

pub fn sections(_role: Role) -> Vec<Section> {
    Section::ALL.to_vec()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord, Hash)]
pub enum View {
    Works,
    Grants,
    Verify,
    Log,
    Alerts,
    MyGrants,
    Persona,
}

impl View {
    pub const ALL: [View; 7] = [View::Works, View::Grants, View::Verify, View::Log, View::Alerts, View::MyGrants, View::Persona];

    pub fn key(self) -> Key {
        match self {
            View::Works => Key::NavWorksView,
            View::Grants => Key::KindGrant,
            View::Verify => Key::NavVerifyView,
            View::Log => Key::NavLogView,
            View::Alerts => Key::NavAlertsView,
            View::MyGrants => Key::NavMyGrantsView,
            View::Persona => Key::PagePersona,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            View::Works => "Works",
            View::Grants => "GrantsView",
            View::Verify => "Verify",
            View::Log => "Log",
            View::Alerts => "Alerts",
            View::MyGrants => "MyGrants",
            View::Persona => "Persona",
        }
    }
}

pub mod tab {
    pub const WORKS_ALL: u8 = 0;
    pub const WORKS_PENDING: u8 = 1;
    pub const WORKS_KIT: u8 = 2;
    pub const GRANTS_LIST: u8 = 0;
    pub const GRANTS_NEW: u8 = 1;
    pub const GRANTS_RELICENSE: u8 = 2;
    pub const VERIFY_CHECK: u8 = 0;
    pub const VERIFY_WORK: u8 = 1;
    pub const VERIFY_OTHERS: u8 = 2;
    pub const HELD_RELICENSE: u8 = 1;
    pub const PERSONA_OVERVIEW: u8 = 0;
    pub const PERSONA_NAMES: u8 = 1;
    pub const PERSONA_HOME: u8 = 2;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Place {
    Home,
    View(View, u8),
    SettingsHome,
    Settings(Section),
    Page(Page),
}

pub type Item = zikaron_ui::nav::Item<Place, Key>;

pub type Group = zikaron_ui::nav::Group<Place, Key>;

pub type History = zikaron_ui::nav::History<Route>;

const fn it(place: Place, name: Key, glyph: Glyph) -> Item {
    Item { place, name, glyph }
}

const HOME: Item = it(Place::Home, Key::NavHome, Glyph::Home);
const VERIFY: Item = it(Place::View(View::Verify, tab::VERIFY_WORK), Key::NavVerifyView, Glyph::Shield);
const ALERTS: Item = it(Place::View(View::Alerts, 0), Key::NavAlertsView, Glyph::Watch);
const PERSONA: Item = it(Place::View(View::Persona, tab::PERSONA_OVERVIEW), Key::PagePersona, Glyph::Person);

pub const AUTHOR: [Group; 3] = [
    Group { title: None, items: &[HOME, PERSONA] },
    Group {
        title: Some(Key::U3Work),
        items: &[
            it(Place::View(View::Works, tab::WORKS_ALL), Key::NavWorksView, Glyph::Doc),
            it(Place::View(View::Grants, tab::GRANTS_LIST), Key::KindGrant, Glyph::Grant),
        ],
    },
    Group { title: Some(Key::NavLook), items: &[VERIFY, it(Place::View(View::Log, 0), Key::NavLogView, Glyph::Queue), ALERTS] },
];

pub const GRANTEE: [Group; 3] = [
    Group { title: None, items: &[HOME] },
    Group {
        title: Some(Key::KindGrant),
        items: &[
            it(Place::View(View::MyGrants, 0), Key::NavMyGrantsView, Glyph::Vault),
        ],
    },
    Group { title: Some(Key::NavLook), items: &[VERIFY, ALERTS] },
];

pub fn rail(role: Role) -> &'static [Group] {
    match role {
        Role::Author => &AUTHOR,
        Role::Grantee => &GRANTEE,
    }
}

pub fn count(role: Role) -> usize {
    rail(role).iter().map(|g| g.items.len()).sum()
}

pub fn tabs(view: View, _role: Role) -> &'static [u8] {
    match view {
        View::Verify => &[tab::VERIFY_WORK, tab::VERIFY_CHECK, tab::VERIFY_OTHERS],
        View::Persona => &[tab::PERSONA_OVERVIEW, tab::PERSONA_NAMES, tab::PERSONA_HOME],
        _ => &[],
    }
}

pub fn home_of(page: Page, _role: Role) -> Place {
    match page {
        Page::FirstRun | Page::Skeleton | Page::About => Place::Settings(Section::About),
        Page::Identity => Place::Settings(Section::Keys),
        Page::Archive => Place::Settings(Section::Data),
        Page::Mirror => Place::Settings(Section::Data),
        Page::Anchoring => Place::View(View::Works, tab::WORKS_ALL),
        Page::Queue => Place::View(View::Works, tab::WORKS_PENDING),
        Page::Kit => Place::View(View::Works, tab::WORKS_KIT),
        Page::Grants | Page::Revoke => Place::View(View::Grants, tab::GRANTS_LIST),
        Page::Grant | Page::FirstWindow => Place::View(View::Grants, tab::GRANTS_NEW),
        Page::Relicense => Place::View(View::MyGrants, tab::HELD_RELICENSE),
        Page::Ledger | Page::Audit => Place::View(View::Log, 0),
        Page::Watch | Page::Sentinel => Place::View(View::Alerts, 0),
        Page::Check => Place::View(View::Verify, tab::VERIFY_CHECK),
        Page::Reader | Page::Diligence => Place::View(View::Verify, tab::VERIFY_OTHERS),
        Page::Verifier => Place::View(View::Verify, tab::VERIFY_WORK),
        Page::Vault | Page::Upstreams | Page::Badge => Place::View(View::MyGrants, 0),
        Page::Delivery => Place::View(View::Verify, tab::VERIFY_CHECK),
        Page::Persona => Place::View(View::Persona, tab::PERSONA_OVERVIEW),
    }
}

pub fn settle(place: Place, role: Role) -> Place {
    let place = match place {
        Place::Page(p) => home_of(p, role),
        other => other,
    };
    let place = match place {
        Place::View(View::Grants, tab::GRANTS_RELICENSE) => Place::View(View::MyGrants, tab::HELD_RELICENSE),
        other => other,
    };
    match place {
        Place::View(v, t) => {
            let on_rail = rail(role).iter().flat_map(|g| g.items.iter()).any(|i| matches!(i.place, Place::View(x, _) if x == v));
            let relicense = v == View::Grants && t == tab::GRANTS_RELICENSE;
            let pending = v == View::Works && t == tab::WORKS_PENDING;
            if !on_rail && !relicense && !pending {
                return Place::Home;
            }
            let ts = tabs(v, role);
            if !ts.is_empty() && !ts.contains(&t) {
                Place::View(v, ts[0])
            } else {
                Place::View(v, t)
            }
        }
        other => other,
    }
}

pub fn lit(place: Place, role: Role) -> Option<Place> {
    match settle(place, role) {
        Place::Settings(_) | Place::SettingsHome | Place::Page(_) => None,
        Place::Home => Some(Place::Home),
        Place::View(v, _) => rail(role)
            .iter()
            .flat_map(|g| g.items.iter())
            .find(|i| matches!(i.place, Place::View(x, _) if x == v))
            .map(|i| i.place),
    }
}

pub const HOME_NAME: &str = "Home";
pub const SETTINGS_NAME: &str = "Settings";
pub const WIZARD_NAME: &str = "Wizard";

const OLD_SECTIONS: [(&str, Section); 5] = [
    ("seat", Section::Data),
    ("chain", Section::Network),
    ("cadence", Section::Notify),
    ("mirror", Section::Data),
    ("backup", Section::Data),
];

pub fn place_named(name: &str) -> Option<(Place, bool)> {
    let n = name.to_ascii_lowercase();
    if n == HOME_NAME.to_ascii_lowercase() {
        return Some((Place::Home, false));
    }
    if n == WIZARD_NAME.to_ascii_lowercase() {
        return Some((Place::Home, true));
    }
    if n == SETTINGS_NAME.to_ascii_lowercase() {
        return Some((Place::SettingsHome, false));
    }
    if let Some(v) = View::ALL.iter().find(|v| v.as_str().to_ascii_lowercase() == n) {
        return Some((Place::View(*v, 0), false));
    }
    let rest = n.strip_prefix(&format!("{}.", SETTINGS_NAME.to_ascii_lowercase()))?;
    if let Some(s) = Section::ALL.iter().find(|s| s.as_str() == rest) {
        return Some((Place::Settings(*s), false));
    }
    OLD_SECTIONS.iter().find(|(o, _)| *o == rest).map(|(_, s)| (Place::Settings(*s), false))
}

pub fn alias_names() -> Vec<String> {
    let mut v = vec![HOME_NAME.to_string(), WIZARD_NAME.to_string(), SETTINGS_NAME.to_string()];
    v.extend(View::ALL.iter().map(|x| x.as_str().to_string()));
    v.extend(Section::ALL.iter().map(|s| format!("{SETTINGS_NAME}.{}", s.as_str())));
    v
}

pub use crate::steps::{prev, Progress, Step, Sub};

#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord, Hash, Default)]
pub enum Stack {
    #[default]
    Home,
    View(View),
    Settings,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Route {
    Root(Place),
    Work(String),
    Pending(String),
    Kit,
    NewGrant,
    Grant(String),
    Entry(String),
    Other(u64),
    Held(String),
    Relicense,
    Section(Section),
}

impl Route {
    pub fn name(&self) -> String {
        match self {
            Route::Root(Place::Home) => HOME_NAME.to_string(),
            Route::Root(Place::SettingsHome) => SETTINGS_NAME.to_string(),
            Route::Root(Place::Settings(s)) | Route::Section(s) => format!("{SETTINGS_NAME}.{}", s.as_str()),
            Route::Root(Place::View(v, t)) => format!("{}.{t}", v.as_str()),
            Route::Root(Place::Page(p)) => format!("{p:?}"),
            Route::Work(_) => "Work".into(),
            Route::Pending(_) => "Pending".into(),
            Route::Kit => "Kit".into(),
            Route::NewGrant => "NewGrant".into(),
            Route::Grant(_) => "Grant".into(),
            Route::Entry(_) => "Entry".into(),
            Route::Other(_) => "OtherEntry".into(),
            Route::Held(_) => "Held".into(),
            Route::Relicense => "Relicense".into(),
        }
    }

    pub fn is_root(&self) -> bool {
        matches!(self, Route::Root(_))
    }
}

pub fn stack_of(place: Place) -> Stack {
    match place {
        Place::Home | Place::Page(_) => Stack::Home,
        Place::View(v, _) => Stack::View(v),
        Place::Settings(_) | Place::SettingsHome => Stack::Settings,
    }
}

pub fn history_of(place: Place, role: Role) -> (Stack, History) {
    let place = settle(place, role);
    let h = match place {
        Place::Settings(s) => History { back: vec![Route::Root(Place::SettingsHome)], cur: Route::Section(s), fwd: Vec::new() },
        Place::View(View::Works, tab::WORKS_KIT) => History { back: vec![Route::Root(Place::View(View::Works, tab::WORKS_ALL))], cur: Route::Kit, fwd: Vec::new() },
        Place::View(View::Grants, tab::GRANTS_NEW) => History { back: vec![Route::Root(Place::View(View::Grants, tab::GRANTS_LIST))], cur: Route::NewGrant, fwd: Vec::new() },
        Place::View(View::MyGrants, tab::HELD_RELICENSE) => History { back: vec![Route::Root(Place::View(View::MyGrants, 0))], cur: Route::Relicense, fwd: Vec::new() },
        other => History::at(Route::Root(other)),
    };
    (stack_of(place), h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_goes_to_the_page_before_then_the_step_before() {
        assert_eq!(prev(Step::Key, Sub::Check), Some((Step::Key, Sub::First)));
        assert_eq!(prev(Step::Pin, Sub::First), None);
        for pair in Step::ALL.windows(2) {
            assert_eq!(prev(pair[1], Sub::First), Some((pair[0], Sub::First)), "{:?}", pair[1]);
            assert_eq!(prev(pair[1], Sub::Check), Some((pair[1], Sub::First)), "{:?}", pair[1]);
        }
    }
}
