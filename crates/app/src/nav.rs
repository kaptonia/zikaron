//! This product's page table: one side rail per role, the views pages are merged into, the settings home and
//! its sections, which history each place belongs to and what it stands on, the first-run wizard's steps, and
//! an alias layer mapping old page names to their new places.
//!
//! This module has no egui dependency. The history and rail mechanism is the window shell's
//! (`zikaron_ui::nav`, generic over the page type); this module only gives it this product's places and
//! routes. The window draws the rail from it and the test driver's `show` resolves old and new names through
//! it, so rail items, grouping, names and old-page mappings have a single source.
//!
//! The set of pages (`shell::Page`) is fixed, since test scripts and documentation refer to it. Views
//! (`View`, the rail items) sit above pages: several pages merge into a view, each as a tab or section.

use crate::lang::Key;
use crate::roles::Role;
use zikaron_ui::icons::Glyph;

/// The pages. Only pages that exist are listed; a page not built is not in the navigation.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Page {
    /// First run: what this machine still needs.
    #[default]
    FirstRun,
    /// Identity and keys.
    Identity,
    /// Archive and single writer.
    Archive,
    /// Mirror and chain.
    Mirror,
    /// Ledger view.
    Ledger,
    /// Disclosure kit output.
    Kit,
    /// Depth.
    Depth,
    /// Grant drafting.
    Grant,
    /// Grant ledger and double-sale guard.
    Grants,
    /// First-window checklist.
    FirstWindow,
    /// Revocation.
    Revoke,
    /// Adoption.
    Adopt,
    /// Succession.
    Succeed,
    /// Reading others' ledgers.
    Reader,
    /// Self-audit clock.
    Audit,
    /// Due diligence.
    Diligence,
    /// Record verification.
    Verifier,
    /// Checking a received record (a tab of the verify page).
    Delivery,
    /// Grant vault.
    Vault,
    /// Revocation sentinel.
    Sentinel,
    /// Multi-upstream ledger.
    Upstreams,
    /// Badges.
    Badge,
    /// Relicensing.
    Relicense,
    /// Grant check (shared, no identity needed).
    Check,
    /// Watch and notifications (shared).
    Watch,
    /// Anchoring desk.
    Anchoring,
    /// Anchor queue.
    Queue,
    /// Skeleton: this shell's own readings.
    Skeleton,
    /// About.
    About,
}

impl Page {
    /// Ordered by family: ledger and anchoring, grants, adoption and succession, disclosure and reading,
    /// machine and settings. Icons follow the family (see [`Page::icon`]): one icon per family, so a repeated
    /// icon marks pages of one family.
    pub const ALL: [Page; 29] = [
        // Ledger and anchoring.
        Page::Ledger,
        Page::Anchoring,
        Page::Queue,
        Page::Audit,
        // Grants.
        Page::Grant,
        Page::Grants,
        Page::Revoke,
        Page::FirstWindow,
        // Adoption and succession.
        Page::Adopt,
        Page::Succeed,
        // Disclosure and reading.
        Page::Kit,
        Page::Depth,
        Page::Reader,
        // Shared.
        Page::Check,
        Page::Watch,
        // Grantee.
        Page::Diligence,
        Page::Verifier,
        Page::Delivery,
        Page::Vault,
        Page::Upstreams,
        Page::Sentinel,
        Page::Relicense,
        Page::Badge,
        // Machine and settings.
        Page::FirstRun,
        Page::Identity,
        Page::Archive,
        Page::Mirror,
        Page::Skeleton,
        Page::About,
    ];

    /// The name this page shows now. Page names come only from here (rail, header, tests).
    ///
    /// Kept separate from `key()` so "page names follow the language" can fail on its own: a broken string
    /// table breaks every sentence, a break here only page names.
    pub fn title(self) -> &'static str {
        crate::lang::t(self.key())
    }

    /// The page name comes from the string table: no sentence is hard-coded in the interface.
    pub fn key(self) -> crate::lang::Key {
        match self {
            Page::FirstRun => crate::lang::Key::PageFirstRun,
            Page::Identity => crate::lang::Key::PageIdentity,
            Page::Archive => crate::lang::Key::PageArchive,
            Page::Mirror => crate::lang::Key::PageMirror,
            Page::Ledger => crate::lang::Key::PageLedger,
            Page::Kit => crate::lang::Key::PageKit,
            Page::Depth => crate::lang::Key::PageDepth,
            Page::Grant => crate::lang::Key::PageGrant,
            Page::Grants => crate::lang::Key::PageGrants,
            Page::FirstWindow => crate::lang::Key::PageFirstWindow,
            Page::Revoke => crate::lang::Key::PageRevoke,
            Page::Adopt => crate::lang::Key::PageAdopt,
            Page::Succeed => crate::lang::Key::PageSucceed,
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
        }
    }

    /// The page icon. One per family: pages of a family share a shape, so a repeat marks the family.
    pub fn icon(self) -> zikaron_ui::icons::Icon {
        use zikaron_ui::icons::Icon;
        match self {
            // Ledger and anchoring.
            Page::Ledger | Page::Anchoring | Page::Queue | Page::Audit => Icon::Ledger,
            // Grants.
            Page::Grant | Page::Grants | Page::Revoke | Page::FirstWindow => Icon::Grant,
            // Adoption and succession.
            Page::Adopt | Page::Succeed => Icon::Anchor,
            // Disclosure and reading.
            Page::Kit | Page::Depth | Page::Reader => Icon::Kit,
            // Grantee: verification and due diligence read bytes from the other party, like the disclosure
            // family.
            Page::Diligence | Page::Verifier | Page::Delivery => Icon::Kit,
            // Shared: checking reads the other party's payload, like the disclosure family.
            Page::Check => Icon::Kit,
            Page::Watch => Icon::Ledger,
            // Grantee credentials hold grants, like the grant family.
            Page::Vault
            | Page::Sentinel
            | Page::Upstreams
            | Page::Badge
            | Page::Relicense => Icon::Grant,
            // Machine and settings.
            Page::FirstRun
            | Page::Identity
            | Page::Archive
            | Page::Mirror
            | Page::Skeleton
            | Page::About => Icon::Gear,
        }
    }
}

/// Settings sections, in top-to-bottom order on the settings home.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Section {
    Language,
    Appearance,
    Keys,
    Network,
    Notify,
    /// Local data: encryption, the whole-machine backup, the ledger mirror (recorder), the data folder.
    Data,
    /// The command line: the terminal command, how it may write this machine's data (through the desktop app or
    /// with a credential), and how its anchoring is handled.
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

    /// The section title's key.
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

    /// The section's line glyph on the settings home.
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

    /// The one-line note under the section's title on the settings home.
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

    /// The name the test driver recognizes (`show Settings.<name>`).
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

/// Which settings sections this role has: both roles have every section (local data holds the backup for
/// both; only its ledger-mirror group is author-only).
pub fn sections(_role: Role) -> Vec<Section> {
    Section::ALL.to_vec()
}

/// Views on the rail.
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

    /// The name the test driver recognizes (`show Works` and so on). Distinct from old page names, which are
    /// matched first.
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

/// Tab (or section) numbers of each view, shared by the window and the alias layer.
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

/// A navigation destination.
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
    /// An old page (translated to a view by [`settle`] before navigating).
    Page(Page),
}

/// One rail item: place, name, glyph.
pub type Item = zikaron_ui::nav::Item<Place, Key>;

/// One rail group: its optional title and its items.
pub type Group = zikaron_ui::nav::Group<Place, Key>;

/// One view's page history.
pub type History = zikaron_ui::nav::History<Route>;

const fn it(place: Place, name: Key, glyph: Glyph) -> Item {
    Item { place, name, glyph }
}

const HOME: Item = it(Place::Home, Key::NavHome, Glyph::Home);
const VERIFY: Item = it(Place::View(View::Verify, tab::VERIFY_CHECK), Key::NavVerifyView, Glyph::Shield);
const ALERTS: Item = it(Place::View(View::Alerts, 0), Key::NavAlertsView, Glyph::Watch);

/// The author's rail: three groups, six items.
pub const AUTHOR: [Group; 3] = [
    Group { title: None, items: &[HOME] },
    Group {
        title: Some(Key::U3Work),
        items: &[
            it(Place::View(View::Works, tab::WORKS_ALL), Key::NavWorksView, Glyph::Doc),
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

/// This role's rail.
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

/// A view's tabs, left to right. Both roles can verify grants, verify records and read others' ledgers:
/// checking a record kit does not depend on the role.
pub fn tabs(view: View, _role: Role) -> &'static [u8] {
    match view {
        View::Verify => &[tab::VERIFY_CHECK, tab::VERIFY_WORK, tab::VERIFY_OTHERS],
        _ => &[],
    }
}

/// The alias layer: where an old page maps to now. Pages merged into a view map to that view's tab; the
/// machine-level pages (first run, identity, archive, mirror, skeleton, about) map to the matching settings
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

/// Resolve a place for a role. Old pages translate to views; a tab this role lacks falls back to the view's
/// first tab; a view not on this role's rail falls back to home (except relicensing and the pending tab,
/// which the grantee uses to sign and anchor relicenses).
pub fn settle(place: Place, role: Role) -> Place {
    let place = match place {
        Place::Page(p) => home_of(p, role),
        other => other,
    };
    // Relicensing lives under my grants; the older tab under grants maps there.
    let place = match place {
        Place::View(View::Grants, tab::GRANTS_RELICENSE) => Place::View(View::MyGrants, tab::HELD_RELICENSE),
        other => other,
    };
    match place {
        Place::View(v, t) => {
            let on_rail = rail(role).iter().flat_map(|g| g.items.iter()).any(|i| matches!(i.place, Place::View(x, _) if x == v));
            let relicense = v == View::Grants && t == tab::GRANTS_RELICENSE;
            // After signing a relicense the grantee must anchor it, so the pending tab opens even though the
            // grantee rail has no work records (it is reached from home and the relicense page).
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

/// Which rail item is highlighted. A view highlights its item (whatever the tab); home highlights home;
/// settings pages highlight the bottom "settings" button (`None`).
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

/// Names the test driver recognizes (old page names are recognized through `Page`).
pub const HOME_NAME: &str = "Home";
pub const SETTINGS_NAME: &str = "Settings";
pub const WIZARD_NAME: &str = "Wizard";

/// Older settings section names still recognized, mapped to their current sections.
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

/// The current names listed when an unknown name is given (after old page names).
pub fn alias_names() -> Vec<String> {
    let mut v = vec![HOME_NAME.to_string(), WIZARD_NAME.to_string(), SETTINGS_NAME.to_string()];
    v.extend(View::ALL.iter().map(|x| x.as_str().to_string()));
    v.extend(Section::ALL.iter().map(|s| format!("{SETTINGS_NAME}.{}", s.as_str())));
    v
}

/// The six steps of the first-run wizard.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    /// Set an eight-character passcode: it opens the key vault, so it comes before holding any key.
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
    /// can wait. Both the "later" and "skip" buttons use this. A grantee does not necessarily need a ledger
    /// (only the first relicense does), so genesis can wait for grantees.
    pub fn deferrable(self, role: Role) -> bool {
        match self {
            Step::Pin => false,
            Step::Key => false,
            // The default is preselected, so this step only needs "next"; no "later".
            Step::Network => false,
            Step::Gas => true,
            Step::Genesis => role == Role::Grantee,
            Step::Backup => true,
        }
    }
}

/// A wizard step's page. The identity step has two once words are made: copying them, then checking the copy;
/// every other step has one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sub {
    /// The step's first page (for the identity step: the words to copy).
    First,
    /// The identity step's check of the copied words.
    Check,
}

/// Where "back" goes from `step` at `sub`, the one place it is decided: a later page of a step goes to the
/// step's first page (the check of the words goes back to the words, unchanged), a first page to the step
/// before it; `None` on the first step's first page.
pub fn prev(step: Step, sub: Sub) -> Option<(Step, Sub)> {
    if sub != Sub::First {
        return Some((step, Sub::First));
    }
    let at = Step::ALL.iter().position(|s| *s == step)?;
    at.checked_sub(1).map(|i| (Step::ALL[i], Sub::First))
}

/// Whether each wizard step is complete, read from real state rather than from which buttons were pressed.
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
    /// The current progress for this role, from the shell's state: the anchor key from the last key store read,
    /// the passcode from the vault, the network and last whole-machine backup from machine settings, gas from
    /// the last chain query, genesis from the root read when the home opened. Used by the window and tests.
    pub fn of(shell: &crate::shell::Shell) -> Progress {
        Progress {
            key: shell.anchor.is_some(),
            pin: !shell.vault.absent(),
            network: shell.machine.network.is_some(),
            gas: matches!(shell.chain_reading(), Ok(Some(crate::task::Done::Chain { gas_wei: Some(w), .. })) if *w > 0),
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

    /// Which step the wizard may show when `want` is requested. Two preconditions, in order:
    ///
    /// 1. Passcode. Everything in the vault is sealed with the master key the passcode opens; without it there
    /// is nowhere to keep a key. Every step falls back to this one until it is done.
    ///
    /// 2. Identity. Until an identity is created (with the recovery-word check passed) or imported, later steps
    /// (gas, genesis, backup) fall back to creating one.
    ///
    /// Creating an identity needs an unlocked vault (`Action::needs_key`), which is why the passcode comes
    /// first.
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

    /// Whether the wizard should open automatically at startup: no anchor key, no passcode, or (for authors)
    /// no genesis yet. Other missing steps only show indicators on the about page.
    pub fn wants_wizard(&self, role: Role) -> bool {
        !self.key || !self.pin || (role == Role::Author && !self.genesis)
    }
}

/// Which page history: each view has its own, as do home and settings.
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

/// Which history a place belongs to.
pub fn stack_of(place: Place) -> Stack {
    match place {
        Place::Home | Place::Page(_) => Stack::Home,
        Place::View(v, _) => Stack::View(v),
        Place::Settings(_) | Place::SettingsHome => Stack::Settings,
    }
}

/// The history to show for a place: the view's own page, or that page with one page pushed (the kit, a new
/// grant, relicensing, a settings section).
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

    /// "Back" from every page of every step: a later page of a step goes to that step's first page (the words'
    /// check to the words), a first page to the step before; nothing before the first step's first page.
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
