//! First run and adoption into the home. The product sets up every precondition itself; an existing ledger
//! directory is adopted entry by entry into this home's sealed ledger, and self-audit runs after adoption.
//!
//! The given directory is only read: not one byte of it changes, and other programs may keep reading it.
//! This home's ledger holds each entry sealed under the local data key (`local::Ledger`), so adopting writes
//! a sealed copy of every entry; a hard link would put plain bytes inside the home, where every local file
//! is sealed.
//!
//! Someone else's directory may contain a README or other programs' files. Detection uses the store crate's
//! lenient read: entries are checked one by one by the core, and anything that cannot be read as an entry is
//! reported with its reason, never treated as absent.
//!
//! A ledger mirror (a folder with the mirror manifest, as "Export ledger mirror…" writes it, this version's or
//! an earlier one's) is adopted too: its entries are read through the mirror's own check ([`mirrored`]), and
//! one entry that fails it refuses the whole mirror.
//!
//! One ledger, one root: what is adopted holds exactly one genesis entry, so the home that takes it has its root as
//! after creating its ledger; entries with no root, or with two, are refused whole before anything is written. One
//! home keeps one ledger: a home that already has a root adopts only that same ledger again (what it holds already
//! is skipped); another ledger is refused whole.

use crate::auditx;
use crate::fault::{Fault, Known};
use crate::home::Home;
use zikaron::entry as k1;
use zikaron::hexfmt;
use zikaron_store::{layout, EntryName, LedgerDir};

/// The reading after inspecting a directory.
pub struct Sighting {
    /// How many were read as entries.
    pub entries: usize,
    /// How many passed the core check one by one.
    pub checked: usize,
    /// Everything that could not be read as an entry (name and reason), reported, never hidden.
    pub skipped: Vec<(String, String)>,
    /// Every entry that failed the core check (name and the spec's token).
    pub refused: Vec<(String, String)>,
}

impl Sighting {
    /// Whether it can be adopted: every entry read passed the check, and there is at least one.
    pub fn adoptable(&self) -> bool {
        self.entries > 0 && self.checked == self.entries && self.refused.is_empty()
    }
}

/// The entries of a ledger mirror, when `dir` is one (it holds the mirror manifest): each checked against the
/// manifest's size and digest, by the core, and against its own name (`mirror::verify`); one that fails refuses
/// the whole mirror, named. `None` when `dir` is not a mirror.
pub fn mirrored(dir: &std::path::Path) -> Result<Option<Vec<Vec<u8>>>, Fault> {
    if !dir.join(zikaron_glue::mirror::MANIFEST).is_file() {
        return Ok(None);
    }
    Ok(Some(crate::mirror::verify(dir)?.into_iter().map(|(_, b)| b).collect()))
}

/// Inspects a directory and re-verifies it entry by entry. Not one byte of that directory changes.
pub fn look(dir: &std::path::Path) -> Result<Sighting, Fault> {
    if let Some(items) = mirrored(dir)? {
        return Ok(Sighting { entries: items.len(), checked: items.len(), skipped: Vec::new(), refused: Vec::new() });
    }
    let ledger = LedgerDir::open(dir).map_err(|t| Fault::known(Known::Ledger, format!("{t:?}")))?;
    let survey = ledger
        .survey()
        .map_err(|t| Fault::known(Known::Ledger, format!("{t:?}")))?;
    let mut out = Sighting {
        entries: survey.items.len(),
        checked: 0,
        skipped: survey
            .skipped
            .iter()
            .map(|s| (s.name.clone(), format!("{:?}", s.why)))
            .collect(),
        refused: Vec::new(),
    };
    for b in &survey.items {
        let name = bare_id(b);
        match k1::check(b) {
            // A file whose name does not match the entry id also fails. File names are spelled by the store
            // crate (`layout::entry_file_name`), not here.
            Ok(_) => match EntryName::parse(&name).map(|n| layout::entry_file_name(&n)) {
                Some(file) => match ledger.read_named(&file) {
                    Ok(same) if &same == b => out.checked += 1,
                    _ => out.refused.push((name, crate::lang::t(crate::lang::Key::Tail109).to_string())),
                },
                None => out.refused.push((name, crate::lang::t(crate::lang::Key::Tail110).to_string())),
            },
            Err(t) => out.refused.push((name, format!("{t:?}"))),
        }
    }
    Ok(out)
}

fn bare_id(b: &[u8]) -> String {
    hexfmt::encode(&k1::entry_id(b)).trim_start_matches("0x").to_string()
}

/// The reading after adoption.
pub struct Adopted {
    pub sighting_entries: usize,
    pub linked: usize,
    /// Self-audit after adoption: the core's label, unchanged.
    pub label: String,
    pub complete: bool,
}

/// Adopts a directory: after inspection, appends every entry, sealed, to this home's ledger, leaving the
/// original directory unchanged (a home that already has a root takes only its own ledger's entries); then runs self-audit once and passes the label through unchanged. A home with
/// the not-fetched mark (an imported key) opens for writing afterwards like any other: once its tail is
/// checked against the chain (`action::check_tail`, started by the action layer after adopting).
pub fn adopt(dir: &std::path::Path, into: &Home) -> Result<Adopted, Fault> {
    let seen = look(dir)?;
    if !seen.adoptable() {
        return Err(Fault::known(
            Known::NotAdoptable,
            crate::lang::filln(crate::lang::Key::Tail111, &[&(seen.entries).to_string(), &(seen.checked).to_string(), &(seen.refused.len()).to_string()]),
        ));
    }
    let items = match mirrored(dir)? {
        Some(items) => items,
        None => {
            let ledger = LedgerDir::open(dir).map_err(|t| Fault::known(Known::Ledger, format!("{t:?}")))?;
            ledger.survey().map_err(|t| Fault::known(Known::Ledger, format!("{t:?}")))?.items
        }
    };
    // One ledger, one root: what is adopted holds exactly one genesis entry (none is some of a ledger's entries, two
    // are two ledgers), so a home that takes it has a root, as after creating its ledger. Refused whole before
    // anything is written.
    let mut roots: Vec<String> = items.iter().filter_map(|b| k1::check(b).ok()).filter(|e| e.seq == 0).map(|e| e.id_hex()).collect();
    roots.sort();
    roots.dedup();
    let whole = roots.len() == 1;
    if !whole {
        return Err(Fault::known(Known::NotAdoptable, format!("roots {} · {}", roots.len(), roots.join(" ")))
            .worded(Some(crate::lang::Key::AdoptOneRoot), Some(crate::lang::Key::AdoptOneRootNext)));
    }
    let theirs = roots.first().cloned().unwrap_or_default();
    // One home, one ledger: a home that already has a root takes only entries of that same ledger (the same again,
    // or what it lacks); a ledger with another root is refused whole before anything is written.
    let ours = into.ledger()?.pile()?.items.iter().filter_map(|b| k1::check(b).ok()).find(|e| e.seq == 0).map(|e| e.id_hex());
    if let Some(ours) = ours {
        if theirs != ours {
            return Err(Fault::known(Known::AlreadyRooted, format!("{ours} · {theirs}"))
                .worded(Some(crate::lang::Key::AdoptOtherRoot), Some(crate::lang::Key::AdoptOtherRootNext)));
        }
    }
    // A ledger handed off to another place is not written here, adopted entries included (`zikaron_seal::pens`).
    let root = items.iter().filter_map(|b| zikaron::entry::check(b).ok()).find(|e| e.seq == 0).map(|e| e.id);
    if let Some(root) = root {
        zikaron_seal::pens::may_write(&crate::home::machine_dir()?, &root)?;
    }
    let book = into.ledger()?;
    let mut linked = 0;
    for b in &items {
        let name = bare_id(b);
        let entry = EntryName::parse(&name)
            .ok_or_else(|| Fault::known(Known::Ledger, crate::lang::filln(crate::lang::Key::Tail012, &[&(name).to_string()])))?;
        // Already in this ledger with the same bytes counts as adopted; different bytes under the same name is
        // refused by name, overwriting nothing (`local::Ledger::append`).
        book.append(&entry, b)?;
        linked += 1;
    }
    // The adopted ledger goes on this machine's blind list (`zikaron_seal::adopted`), so the command line refuses
    // to write its folder from now on; the folder itself is not touched.
    if let Some(root) = root {
        zikaron_seal::adopted::mark(&crate::home::machine_dir()?, &root)?;
    }
    let v = auditx::offline(into)?;
    Ok(Adopted {
        sighting_entries: seen.entries,
        linked,
        label: v.label,
        complete: v.complete,
    })
}

// ───────────────────────── First-run checklist ─────────────────────────

/// The author seat's first-run points.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Author {
    AnchorKey,
    /// Whether a passcode is set (no passcode means no key vault).
    Pin,
    GasFloat,
    Genesis,
    /// A whole-machine backup exists and nothing came after it.
    Backup,
}

impl Author {
    pub const ALL: [Author; 5] = [
        Author::AnchorKey,
        Author::Pin,
        Author::GasFloat,
        Author::Genesis,
        Author::Backup,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Author::AnchorKey => "anchor_key",
            Author::Pin => "pin",
            Author::GasFloat => "gas_float",
            // App names never share spelling with spec tokens. This first-run point asks "does this ledger
            // have a root yet"; spelled like the spec's entry kind, a reader could not tell a spec literal
            // from the app's own name.
            Author::Genesis => "rooted",
            Author::Backup => "backup",
        }
    }
}

/// The grantee seat's first-run points.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Grantee {
    AnchorKey,
    Pin,
    Endpoints,
    Ready,
    /// A whole-machine backup exists and nothing came after it.
    Backup,
}

impl Grantee {
    pub const ALL: [Grantee; 5] = [
        Grantee::AnchorKey,
        Grantee::Pin,
        Grantee::Endpoints,
        Grantee::Ready,
        Grantee::Backup,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Grantee::AnchorKey => "anchor_key",
            Grantee::Pin => "pin",
            Grantee::Endpoints => "endpoints",
            Grantee::Ready => "ready",
            Grantee::Backup => "backup",
        }
    }
}

// ───────────────────────── The mirror slot point ─────────────────────────

/// A point's four colours. Grey is not a kind of bad: never backed up, or backed up and not measured yet.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shade {
    /// Complete: the last backup holds everything there is now.
    Green,
    /// Backed up, but more has been written since.
    Amber,
    /// Never backed up; or backed up, and how much is here now has not been measured yet.
    Grey,
    /// The last backup tried was not written, or did not read back.
    Red,
}

impl Shade {
    pub fn as_str(self) -> &'static str {
        match self {
            Shade::Green => "green",
            Shade::Amber => "amber",
            Shade::Grey => "grey",
            Shade::Red => "red",
        }
    }
}

/// Colour of the whole-machine backup point (the setup check's last point, the wizard's sixth step). Pure:
/// reads no disk, only compares the machine settings' record of the last backup with the latest measured
/// count (ledger entries and held grants).
///
/// Red only for a backup attempt that failed (`failed`: not written, or not read back). Never backed up is
/// grey (not done yet); backed up but behind is amber; backed up with nothing measured yet is grey ("done,
/// unclear now").
pub fn backup_point(last: Option<&crate::machine::Backed>, now: Option<u64>, failed: Option<u64>) -> Shade {
    if failed.is_some() {
        return Shade::Red;
    }
    match (last, crate::machine::backup_behind(last, now)) {
        (None, _) => Shade::Grey,
        (Some(_), None) => Shade::Grey,
        (Some(_), Some(n)) if n > 0 => Shade::Amber,
        (Some(_), Some(_)) => Shade::Green,
    }
}
