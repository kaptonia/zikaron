//! First run and adoption into the home. Every precondition is built by the product; an existing ledger
//! directory is adopted entry by entry into this home's sealed ledger, and self-audit runs after adoption.
//!
//! ─── What adopting leaves where ───
//!
//! The directory someone gave is read and never written: not one byte of it changes, and other programs may
//! go on reading it. This home's ledger holds each entry sealed under the local data key (`local::Ledger`),
//! so adopting writes a sealed copy of every entry; a hard link would put the plain bytes inside this home,
//! where every local file is sealed.
//!
//! ─── Foreign layouts are read ───
//!
//! Someone else's directory may contain a README or other programs' things. Detection uses the store crate's
//! lenient read: entries read are checked one by one through the core, and everything that cannot be read as
//! an entry is reported with its reason, never treated as absent.

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
    /// Every entry that failed the core check (name and the law's token).
    pub refused: Vec<(String, String)>,
}

impl Sighting {
    /// Whether it can be adopted: every entry read passed the check, and there is at least one.
    pub fn adoptable(&self) -> bool {
        self.entries > 0 && self.checked == self.entries && self.refused.is_empty()
    }
}

/// Inspect and re-verify entry by entry. Not one byte of that directory changes.
pub fn look(dir: &std::path::Path) -> Result<Sighting, Fault> {
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
            // A name that does not match the id also fails: that entry has the wrong name in this directory.
            // File name spelling comes from the store crate (`layout::entry_file_name`); this layer spells
            // nothing itself.
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

/// Adopt. After inspection, append every entry, sealed, to this home's ledger; not one byte of the original
/// directory changes. After landing, run self-audit once and pass the label through unchanged.
pub fn adopt(dir: &std::path::Path, into: &Home) -> Result<Adopted, Fault> {
    let seen = look(dir)?;
    if !seen.adoptable() {
        return Err(Fault::known(
            Known::NotAdoptable,
            crate::lang::filln(crate::lang::Key::Tail111, &[&(seen.entries).to_string(), &(seen.checked).to_string(), &(seen.refused.len()).to_string()]),
        ));
    }
    let ledger = LedgerDir::open(dir).map_err(|t| Fault::known(Known::Ledger, format!("{t:?}")))?;
    let survey = ledger
        .survey()
        .map_err(|t| Fault::known(Known::Ledger, format!("{t:?}")))?;
    let book = into.ledger()?;
    let mut linked = 0;
    for b in &survey.items {
        let name = bare_id(b);
        let entry = EntryName::parse(&name)
            .ok_or_else(|| Fault::known(Known::Ledger, crate::lang::filln(crate::lang::Key::Tail012, &[&(name).to_string()])))?;
        // Already in this ledger with the same bytes counts as adopted; different bytes under the same name is
        // refused by name, overwriting nothing (`local::Ledger::append`).
        book.append(&entry, b)?;
        linked += 1;
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
            // The product's own names do not share a form with the law's words. This one names one of the
            // first-run points (the same family as anchor_key / gas_float / backup), and it asks
            // "does this ledger have a root yet". Spelled as the law's entry kind literal, a reader could not
            // tell whether it is the law's literal or the shell's name, and the law's literals live only in
            // the base. With its own name, this family needs no exemption from that rule.
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

/// A point's three colors. Gray is not a kind of bad; it means "existed, but unclear right now".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shade {
    /// Complete: the last backup holds everything there is now.
    Green,
    /// Backed up, but more has been written since.
    Amber,
    /// Backed up, and how much is here now has not been measured yet.
    Grey,
    /// Never backed up.
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

/// A true first run: this machine has no passcode and no identity yet (nothing to go back to). The wizard
/// asks this once, as it opens, to decide whether it offers a way out for the whole run.
pub fn fresh_machine(shell: &crate::shell::Shell) -> bool {
    let pin = !matches!(shell.vault, crate::keybox::State::Absent);
    let identity = shell.identities.as_ref().map(|r| !r.rows.is_empty()).unwrap_or(false);
    !pin && !identity
}

/// What color the whole-machine backup point reads (the setup check's last point, the wizard's sixth step). A
/// pure function: it asks no disk, only compares the machine settings' record of the last backup with the
/// count the last measurement brought back (ledger entries and held grants).
///
/// Red is kept for "never backed up", something the person has not done yet; backed up but behind is amber;
/// backed up with nothing measured yet is grey ("done, unclear now"), not red.
pub fn backup_point(last: Option<&crate::machine::Backed>, now: Option<u64>) -> Shade {
    match (last, crate::machine::backup_behind(last, now)) {
        (None, _) => Shade::Red,
        (Some(_), None) => Shade::Grey,
        (Some(_), Some(n)) if n > 0 => Shade::Amber,
        (Some(_), Some(_)) => Shade::Green,
    }
}
