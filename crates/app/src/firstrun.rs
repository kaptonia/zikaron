use crate::auditx;
use crate::fault::{Fault, Known};
use crate::home::Home;
use zikaron::hexfmt;
use zikaron_glue::book;
use zikaron_store::{layout, EntryName, LedgerDir};

pub struct Sighting {
    pub entries: usize,
    pub checked: usize,
    pub skipped: Vec<(String, String)>,
    pub refused: Vec<(String, String)>,
}

impl Sighting {
    pub fn adoptable(&self) -> bool {
        self.entries > 0 && self.checked == self.entries && self.refused.is_empty()
    }
}

pub fn mirrored(dir: &std::path::Path) -> Result<Option<Vec<Vec<u8>>>, Fault> {
    if !dir.join(zikaron_glue::mirror::MANIFEST).is_file() {
        return Ok(None);
    }
    Ok(Some(crate::mirror::verify(dir)?.into_iter().map(|(_, b)| b).collect()))
}

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
        match book::check(b) {
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
    hexfmt::encode(&book::entry_id(b)).trim_start_matches("0x").to_string()
}

pub struct Adopted {
    pub sighting_entries: usize,
    pub linked: usize,
    pub label: String,
    pub complete: bool,
}

pub fn adopt(dir: &std::path::Path, into: &Home) -> Result<Adopted, Fault> {
    let seen = look(dir)?;
    if !seen.adoptable() {
        return Err(Fault::known(
            Known::NotAdoptable,
            crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail111, &[&(seen.entries).to_string(), &(seen.checked).to_string(), &(seen.refused.len()).to_string()])),
        ));
    }
    let items = match mirrored(dir)? {
        Some(items) => items,
        None => {
            let ledger = LedgerDir::open(dir).map_err(|t| Fault::known(Known::Ledger, format!("{t:?}")))?;
            ledger.survey().map_err(|t| Fault::known(Known::Ledger, format!("{t:?}")))?.items
        }
    };
    if let Err(n) = book::root_of(&items) {
        let count = match n {
            book::NoRoot::Empty => 0,
            book::NoRoot::Forked(n) => n,
        };
        return Err(Fault::known(Known::NotAdoptable, format!("roots {count}"))
            .worded(Some(crate::lang::Key::AdoptOneRoot), Some(crate::lang::Key::AdoptOneRootNext)));
    }
    let theirs = book::root_entry(&items).map(|e| e.id_hex()).unwrap_or_default();
    let ours = book::root_entry(&into.ledger()?.pile()?.items).map(|e| e.id_hex());
    if let Some(ours) = ours {
        if theirs != ours {
            return Err(Fault::known(Known::AlreadyRooted, format!("{ours} · {theirs}"))
                .worded(Some(crate::lang::Key::AdoptOtherRoot), Some(crate::lang::Key::AdoptOtherRootNext)));
        }
    }
    let have = into.ledger()?.pile()?.items;
    let here: Vec<book::Entry> = have.iter().filter_map(|b| book::check(b).ok()).collect();
    let twins = items.iter().filter_map(|b| book::check(b).ok()).filter(|t| here.iter().any(|h| h.law == t.law && h.seq == t.seq && h.id != t.id)).count();
    if twins > 0 {
        return Err(Fault::known(Known::Broken, format!("twins {twins}")).worded(Some(crate::lang::Key::AdoptTwinEntries), Some(crate::lang::Key::AdoptTwinEntriesNext)));
    }
    let root = book::root_entry(&items).map(|e| e.id);
    if let Some(root) = root {
        zikaron_seal::pens::may_write(&crate::home::machine_dir()?, &root)?;
    }
    let book = into.ledger()?;
    let mut linked = 0;
    for b in &items {
        let name = bare_id(b);
        let entry = EntryName::parse(&name)
            .ok_or_else(|| Fault::known(Known::Ledger, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail012, &[&(name).to_string()]))))?;
        book.append(&entry, b)?;
        linked += 1;
    }
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Author {
    AnchorKey,
    Pin,
    GasFloat,
    Genesis,
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
            Author::Genesis => "rooted",
            Author::Backup => "backup",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Grantee {
    AnchorKey,
    Pin,
    Endpoints,
    Ready,
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

pub use crate::backup::{backup_point, Shade};
