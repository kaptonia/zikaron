//! Navigation: one side rail per seat, the views after merging pages, the settings home and its sections, a
//! history of pages per view, and an alias layer that lands old page names on their new places.
//!
//! This layer knows where to go, not egui. The window draws the rail and lands old pages by it; the test
//! driver's `show` recognizes both old and new names by it. So how many rail items, how they group and what
//! they are called, and where an old page name lands, each have one source that the window and the test
//! driver both read.
//!
//! Pages (`shell::Page`) are neither added nor removed: the test driver's step lists, the manual and the audit book
//! count pages by that closed table. Above pages are views (`View`, the rail items): several pages merge into a view,
//! each taking a tab or a section.

use crate::lang::Key;
use crate::roles::Role;
use crate::shell::Page;
use zikaron_ui::icons::Glyph;

/// Settings sections. Closed; the order is top to bottom on the settings home.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Section {
    Language,
    Appearance,
    Keys,
    Network,
    Notify,
    /// Local data: encryption, the whole-machine backup, the ledger mirror (recorder), the data folder.
    Data,
    About,
}

impl Section {
    pub const ALL: [Section; 7] = [
        Section::Language,
        Section::Appearance,
        Section::Keys,
        Section::Network,
        Section::Notify,
        Section::Data,
        Section::About,
    ];

    /// The section title's key.
    pub fn key(self) -> Key {
        match self {
            Section::Language => Key::SetLangTime,
            Section::Appearance => Key::SetAppearance,
            Section::Keys => Key::SetKeys,
            Section::Network => Key::SetChain,
            Section::Notify => Key::SetCadence,
            Section::Data => Key::SetData,
            Section::About => Key::PageAbout,
        }
    }

    /// The one line under the section's title on the settings home.
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
            Section::About => Key::SetNoteAbout,
        }
    }

    /// The name the test driver recognizes (`show Settings.<name>`).
    pub fn as_str(self) -> &'static str {
        match self {
            Section::Language => "language",
            Section::Appearance => "appearance",
            Section::Keys => "keys",
            Section::Network => "network",
            Section::Notify => "notify",
            Section::Data => "data",
            Section::About => "about",
        }
    }
}

/// Which settings sections this role has: both seats have every section (local data holds the backup for both;
/// only its ledger-mirror group is the recorder's).
pub fn sections(_role: Role) -> Vec<Section> {
    Section::ALL.to_vec()
}

/// Views on the rail. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord, Hash)]
pub enum View {
    /// Work records (anchoring and the queue merged in; kits and depth are destinations from record details).
    Works,
    /// Grants (drafting and the table merged in; revocation at the bottom of the detail card).
    Grants,
    /// Verify (grant check, record verification, received records, others' records).
    Verify,
    /// Records (the ledger page; self-audit, adoption, succession and notes under its "more" and status bar).
    Log,
    /// Alerts (watch and the revocation sentinel merged in).
    Alerts,
    /// My grants (the vault; badges and relicensing on the detail card).
    MyGrants,
}

impl View {
    pub const ALL: [View; 6] = [View::Works, View::Grants, View::Verify, View::Log, View::Alerts, View::MyGrants];

    pub fn key(self) -> Key {
        match self {
            View::Works => Key::NavWorksView,
            View::Grants => Key::KindGrant,
            View::Verify => Key::NavVerifyView,
            View::Log => Key::NavLogView,
            View::Alerts => Key::NavAlertsView,
            View::MyGrants => Key::NavMyGrantsView,
        }
    }

    /// The new name the test driver recognizes (`show Works` and so on). Does not collide with old page names
    /// (those are matched first).
    pub fn as_str(self) -> &'static str {
        match self {
            View::Works => "Works",
            View::Grants => "GrantsView",
            View::Verify => "Verify",
            View::Log => "Log",
            View::Alerts => "Alerts",
            View::MyGrants => "MyGrants",
        }
    }
}

/// Tab (or section) numbers of each view, defined once: the window, the alias layer and old page landings use
/// these names.
pub mod tab {
    /// Work records: all.
    pub const WORKS_ALL: u8 = 0;
    /// Work records: to be anchored.
    pub const WORKS_PENDING: u8 = 1;
    /// Work records: export kit (a destination from record details).
    pub const WORKS_KIT: u8 = 2;
    /// Grants: list.
    pub const GRANTS_LIST: u8 = 0;
    /// Grants: new grant.
    pub const GRANTS_NEW: u8 = 1;
    /// Grants: relicense (the grantee arrives from a my-grants detail card).
    pub const GRANTS_RELICENSE: u8 = 2;
    /// Verify: grant check.
    pub const VERIFY_CHECK: u8 = 0;
    /// Verify: record verification (grantee only).
    pub const VERIFY_WORK: u8 = 1;
    /// Verify: others' records.
    pub const VERIFY_OTHERS: u8 = 2;
    /// My grants: relicensing a held grant (a page pushed from that grant's detail).
    pub const HELD_RELICENSE: u8 = 1;
}

/// A place.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Place {
    /// Home.
    Home,
    /// A view, at one of its tabs.
    View(View, u8),
    /// The settings home: the list of sections.
    SettingsHome,
    /// One settings section's page.
    Settings(Section),
    /// An old page (entry form: translated to a view by [`settle`] before landing).
    Page(Page),
}

/// One rail item: place, name, glyph.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Item {
    pub place: Place,
    pub name: Key,
    pub glyph: Glyph,
}

/// One rail group: its title (`None` for none) and items.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Group {
    pub title: Option<Key>,
    pub items: &'static [Item],
}

const fn it(place: Place, name: Key, glyph: Glyph) -> Item {
    Item { place, name, glyph }
}

const HOME: Item = it(Place::Home, Key::NavHome, Glyph::Home);
const VERIFY: Item = it(Place::View(View::Verify, tab::VERIFY_CHECK), Key::NavVerifyView, Glyph::Check);
const ALERTS: Item = it(Place::View(View::Alerts, 0), Key::NavAlertsView, Glyph::Watch);

/// The author's rail: three groups, six items.
pub const AUTHOR: [Group; 3] = [
    Group { title: None, items: &[HOME] },
    Group {
        title: Some(Key::U3Work),
        items: &[
            it(Place::View(View::Works, tab::WORKS_ALL), Key::NavWorksView, Glyph::Ledger),
            it(Place::View(View::Grants, tab::GRANTS_LIST), Key::KindGrant, Glyph::Grant),
        ],
    },
    Group { title: Some(Key::NavLook), items: &[VERIFY, it(Place::View(View::Log, 0), Key::NavLogView, Glyph::Queue), ALERTS] },
];

/// The grantee's rail: three groups, four items (the delivery item merged into the verify page).
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

/// This role's rail. Switching role switches the whole rail, because the rail comes only from here.
pub fn rail(role: Role) -> &'static [Group] {
    match role {
        Role::Author => &AUTHOR,
        Role::Grantee => &GRANTEE,
    }
}

/// How many rail items this role has (author 6, grantee 4).
pub fn count(role: Role) -> usize {
    rail(role).iter().map(|g| g.items.len()).sum()
}

/// The closed tab table of a view (left to right on the page). Both seats verify grants, verify records and
/// read others' ledgers: checking a record kit asks nothing of the seat.
pub fn tabs(view: View, _role: Role) -> &'static [u8] {
    match view {
        View::Verify => &[tab::VERIFY_CHECK, tab::VERIFY_WORK, tab::VERIFY_OTHERS],
        _ => &[],
    }
}

/// The alias layer: where an old page lands now. Pages merged into a view land on that view's tab; the
/// machine family (first run, identity, archive, mirror, skeleton, about) lands on the matching settings
/// section.
pub fn home_of(page: Page, _role: Role) -> Place {
    match page {
        Page::FirstRun | Page::Skeleton | Page::About => Place::Settings(Section::About),
        Page::Identity => Place::Settings(Section::Keys),
        Page::Archive => Place::Settings(Section::Data),
        Page::Mirror => Place::Settings(Section::Data),
        Page::Anchoring | Page::Depth => Place::View(View::Works, tab::WORKS_ALL),
        Page::Queue => Place::View(View::Works, tab::WORKS_PENDING),
        Page::Kit => Place::View(View::Works, tab::WORKS_KIT),
        Page::Grants | Page::Revoke => Place::View(View::Grants, tab::GRANTS_LIST),
        Page::Grant | Page::FirstWindow => Place::View(View::Grants, tab::GRANTS_NEW),
        Page::Relicense => Place::View(View::MyGrants, tab::HELD_RELICENSE),
        Page::Ledger | Page::Audit | Page::Adopt | Page::Succeed => Place::View(View::Log, 0),
        Page::Watch | Page::Sentinel => Place::View(View::Alerts, 0),
        Page::Check => Place::View(View::Verify, tab::VERIFY_CHECK),
        Page::Reader | Page::Diligence => Place::View(View::Verify, tab::VERIFY_OTHERS),
        Page::Verifier => Place::View(View::Verify, tab::VERIFY_WORK),
        Page::Vault | Page::Upstreams | Page::Badge => Place::View(View::MyGrants, 0),
        // Checking a received record is part of the grant check page.
        Page::Delivery => Place::View(View::Verify, tab::VERIFY_CHECK),
    }
}

/// How a place settles. Old page entries translate to views; a tab this role lacks lands on the view's first
/// tab; a view not on this role's rail lands on home (except relicensing and the pending tab: the grantee
/// signs and anchors relicenses through these two).
pub fn settle(place: Place, role: Role) -> Place {
    let place = match place {
        Place::Page(p) => home_of(p, role),
        other => other,
    };
    // Relicensing lives under my grants now; the older tab under grants lands there.
    let place = match place {
        Place::View(View::Grants, tab::GRANTS_RELICENSE) => Place::View(View::MyGrants, tab::HELD_RELICENSE),
        other => other,
    };
    match place {
        Place::View(v, t) => {
            let on_rail = rail(role).iter().flat_map(|g| g.items.iter()).any(|i| matches!(i.place, Place::View(x, _) if x == v));
            let relicense = v == View::Grants && t == tab::GRANTS_RELICENSE;
            // After signing a relicense the grantee needs to anchor it: the pending tab opens anyway (the
            // grantee rail has no work records; the entry is on home and the relicense page).
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

/// Which rail item is lit. A view lights its item (whatever the tab); home lights home; every settings
/// section lights the bottom "settings" (`None`).
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

/// The new name the test driver recognizes (old page names are recognized by `Page`).
pub const HOME_NAME: &str = "Home";
pub const SETTINGS_NAME: &str = "Settings";
pub const WIZARD_NAME: &str = "Wizard";

/// Older settings section names still recognized, landing on their current sections.
const OLD_SECTIONS: [(&str, Section); 5] = [
    ("seat", Section::Data),
    ("chain", Section::Network),
    ("cadence", Section::Notify),
    ("mirror", Section::Data),
    ("backup", Section::Data),
];

/// The alias layer: `Home`, `Wizard`, `Settings`, `Settings.<section>` (old and new section names) and
/// the view names. Returns the place and whether to open the wizard.
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

/// The new names listed when a name is refused (after old page names).
pub fn alias_names() -> Vec<String> {
    let mut v = vec![HOME_NAME.to_string(), WIZARD_NAME.to_string(), SETTINGS_NAME.to_string()];
    v.extend(View::ALL.iter().map(|x| x.as_str().to_string()));
    v.extend(Section::ALL.iter().map(|s| format!("{SETTINGS_NAME}.{}", s.as_str())));
    v
}

/// The six steps of the first-run wizard. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    /// Set an eight-character passcode: the key vault opens with it, so it comes before holding any key.
    Pin,
    /// Create an identity (generate twelve words and confirm the copy, or import an existing key).
    Key,
    /// Choose a network (a row of the known deployments table, or custom; saved to machine settings). The
    /// default row is preselected, so pressing next chooses it.
    Network,
    /// Write genesis (required, before funding gas).
    Genesis,
    /// Fund a little gas (can be done later).
    Gas,
    /// A whole-machine backup (can be done later): one file that restores everything.
    Backup,
}

impl Step {
    pub const ALL: [Step; 6] = [Step::Pin, Step::Key, Step::Network, Step::Genesis, Step::Gas, Step::Backup];

    pub fn title(self) -> Key {
        match self {
            Step::Key => Key::WizKeyTitle,
            Step::Pin => Key::WizPinTitle,
            Step::Network => Key::SetChain,
            Step::Gas => Key::WizGasTitle,
            Step::Genesis => Key::WizGenesisTitle,
            Step::Backup => Key::WizBackupTitle,
        }
    }

    /// Whether this step can be done later. Passcode, identity and genesis are required; only gas and backup
    /// location can wait. Closed: the "later" key and the skip key when incomplete both ask this. A grantee
    /// does not necessarily need a ledger (only the first relicense does), so genesis can wait for the
    /// grantee.
    pub fn deferrable(self, role: Role) -> bool {
        match self {
            Step::Pin => false,
            Step::Key => false,
            // The default row is preselected: this step only needs pressing next, so there is no "later".
            Step::Network => false,
            Step::Gas => true,
            Step::Genesis => role == Role::Grantee,
            Step::Backup => true,
        }
    }
}

/// The wizard's reading: whether each step is complete now. Read from real state, not from "next was
/// pressed".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Progress {
    pub key: bool,
    /// Whether this machine has a passcode (the vault has its seal).
    pub pin: bool,
    /// Whether this machine chose a network (whether the machine settings field exists).
    pub network: bool,
    pub gas: bool,
    pub genesis: bool,
    /// Whether this machine has a whole-machine backup (the machine settings record the last one).
    pub backup: bool,
}

impl Progress {
    /// This seat's wizard reading now, from the shell's real state: the anchor key from the last key store
    /// reading, the passcode from the vault cell, the network and the last whole-machine backup from the
    /// machine settings, gas from the last chain query, genesis from the root read when the home opened. The
    /// one place these facts are gathered (the window and the tests both read it).
    pub fn of(shell: &crate::shell::Shell) -> Progress {
        Progress {
            key: shell.anchor.is_some(),
            pin: !shell.vault.absent(),
            network: shell.machine.network.is_some(),
            gas: matches!(&shell.chain, Some(crate::task::Done::Chain { gas_wei: Some(w), .. }) if *w > 0),
            genesis: shell.rooted,
            backup: shell.machine.backup.is_some(),
        }
    }

    pub fn done(&self, s: Step) -> bool {
        match s {
            Step::Key => self.key,
            Step::Pin => self.pin,
            Step::Network => self.network,
            Step::Gas => self.gas,
            Step::Genesis => self.genesis,
            Step::Backup => self.backup,
        }
    }

    /// Which step the wizard can stand on. Two preconditions, in order:
    ///
    /// 1. Passcode. Every seal in the vault is made with the master key the passcode opens; without a
    /// passcode there is no vault and nowhere to keep a key. So setting it is the first step, and any step
    /// falls back to it until it is done.
    ///
    /// 2. Identity. Without an identity built through verification (the three words passed the action layer's
    /// check) or imported, the later steps (gas, genesis, mirror) fall back to creating one.
    ///
    /// Creating an identity uses a key (`Action::needs_key`), which needs an open vault; with identity first,
    /// a new machine would stand on that step, the action layer would answer "the vault is locked", and the
    /// step that opens the vault would come after it. The order itself fixes this: vault first, then keys.
    pub fn gate(&self, want: Step) -> Step {
        if want != Step::Pin && !self.pin {
            Step::Pin
        } else if !matches!(want, Step::Pin | Step::Key) && !self.key {
            Step::Key
        } else {
            want
        }
    }

    /// The first incomplete step; `None` when all are complete.
    pub fn first_open(&self) -> Option<Step> {
        Step::ALL.iter().copied().find(|s| !self.done(*s))
    }

    /// Whether the wizard should open by itself at start: no anchor key, or this seat's evidence ledger not started.
    /// Other missing steps only light lamps in about.
    pub fn wants_wizard(&self, role: Role) -> bool {
        !self.key || !self.pin || (role == Role::Author && !self.genesis)
    }
}

/// Each view keeps its own history of pages; home and settings keep theirs.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord, Hash, Default)]
pub enum Stack {
    #[default]
    Home,
    View(View),
    Settings,
}

/// One page in a view's history. Detail pages carry what they show (an entry id, a sequence number).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Route {
    /// The view's own page: home, a view at one of its tabs, the settings home.
    Root(Place),
    /// A record's detail, by its anchoring entry's id.
    Work(String),
    /// An entry waiting to be anchored, by its id.
    Pending(String),
    /// Export a record kit (the chosen entries live in the form).
    Kit,
    /// Draft a new grant.
    NewGrant,
    /// A grant's detail, by its entry id (a grant opened from the ledger shows the same page).
    Grant(String),
    /// A ledger entry's detail, by its id.
    Entry(String),
    /// An entry of someone else's ledger (read-only), by its sequence number.
    Other(u64),
    /// A grant I hold, by its id.
    Held(String),
    /// Relicense the grant I hold (the upstream grant lives in the form).
    Relicense,
    /// One settings section.
    Section(Section),
}

impl Route {
    /// The page's name (tests read which page a frame drew).
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

    /// Whether this is a view's own page (not a pushed detail).
    pub fn is_root(&self) -> bool {
        matches!(self, Route::Root(_))
    }
}

/// A view's history: pages behind, the page shown, pages ahead.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct History {
    pub back: Vec<Route>,
    pub cur: Route,
    pub fwd: Vec<Route>,
}

impl History {
    /// A history standing on a view's own page.
    pub fn at(root: Place) -> History {
        History { back: Vec::new(), cur: Route::Root(root), fwd: Vec::new() }
    }

    /// Go into a page: the current one goes behind, pages ahead are dropped.
    pub fn push(&mut self, r: Route) {
        let was = std::mem::replace(&mut self.cur, r);
        self.back.push(was);
        self.fwd.clear();
    }

    /// Back one page; false when there is none.
    pub fn back(&mut self) -> bool {
        match self.back.pop() {
            Some(prev) => {
                let was = std::mem::replace(&mut self.cur, prev);
                self.fwd.insert(0, was);
                true
            }
            None => false,
        }
    }

    /// Forward one page; false when there is none.
    pub fn fwd(&mut self) -> bool {
        if self.fwd.is_empty() {
            return false;
        }
        let next = self.fwd.remove(0);
        let was = std::mem::replace(&mut self.cur, next);
        self.back.push(was);
        true
    }

    /// Back to the view's own page, forgetting the history.
    pub fn root(&mut self, root: Place) {
        *self = History::at(root);
    }

    /// The view's own page at the bottom of the history (the first page behind, or the current one).
    pub fn bottom(&self) -> &Route {
        self.back.first().unwrap_or(&self.cur)
    }

    /// Whether there is anything behind or ahead.
    pub fn has_history(&self) -> bool {
        !self.back.is_empty() || !self.fwd.is_empty()
    }
}

/// Which history a place belongs to.
pub fn stack_of(place: Place) -> Stack {
    match place {
        Place::Home | Place::Page(_) => Stack::Home,
        Place::View(v, _) => Stack::View(v),
        Place::Settings(_) | Place::SettingsHome => Stack::Settings,
    }
}

/// The history a place lands as: a view's own page, or its own page with one page pushed (the kit, a new
/// grant, relicensing, a settings section).
pub fn history_of(place: Place, role: Role) -> (Stack, History) {
    let place = settle(place, role);
    let h = match place {
        Place::Settings(s) => History { back: vec![Route::Root(Place::SettingsHome)], cur: Route::Section(s), fwd: Vec::new() },
        Place::View(View::Works, tab::WORKS_KIT) => History { back: vec![Route::Root(Place::View(View::Works, tab::WORKS_ALL))], cur: Route::Kit, fwd: Vec::new() },
        Place::View(View::Grants, tab::GRANTS_NEW) => History { back: vec![Route::Root(Place::View(View::Grants, tab::GRANTS_LIST))], cur: Route::NewGrant, fwd: Vec::new() },
        Place::View(View::MyGrants, tab::HELD_RELICENSE) => History { back: vec![Route::Root(Place::View(View::MyGrants, 0))], cur: Route::Relicense, fwd: Vec::new() },
        other => History::at(other),
    };
    (stack_of(place), h)
}
