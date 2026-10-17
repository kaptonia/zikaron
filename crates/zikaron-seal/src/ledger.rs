//! A home's ledger: the store crate's ledger directory holding sealed entries, opened and written with the keys
//! and the home's number handed in ([`Keys`]). The bytes returned are the entries' own; what they say is the core's
//! business.

use crate::envelope::{open_with, seal_checked, Doc, Expect, Ident, Owner, LEDGER_DIR};
use crate::keybox::LocalKey;
use crate::names::{Logical, NameKey};
use crate::trouble::{Code, Trouble};
use std::path::{Path, PathBuf};
use zikaron_store::{EntryName, LedgerDir, Pile, Skip, Stored, Survey};

/// What a read or write of one home's ledger needs: the local data key, the names key, and the home's owner (its
/// label's number; `None` when the home has no label, so no entry can belong to it).
pub struct Keys<'a> {
    pub local: &'a LocalKey,
    pub names: &'a NameKey,
    pub owner: Option<Owner>,
}

fn store(t: zikaron_store::Trouble) -> Trouble {
    Trouble::new(Code::Ledger, format!("{t:?}"))
}

/// The marker for empty entry files (zero bytes under an entry's name) in a strict read's refusal. The store
/// skips such files as interrupted writes; a strict read refuses them, because a pile missing that entry would
/// look complete downstream and audit differently.
pub const EMPTY: &str = "EMPTY";

/// A strict read's refusal naming the empty entry files ([`EMPTY`] and each on-disk name).
pub fn empty_trouble(names: &[String]) -> Trouble {
    Trouble::new(Code::Ledger, format!("{EMPTY} {}", names.join(" ")))
}

/// Names of the empty entry files a lenient read skipped (`IN_FLIGHT` under an entry's name).
pub fn empty_in(skipped: &[Skip]) -> Vec<String> {
    skipped
        .iter()
        .filter(|k| k.why == zikaron_store::Why::InFlight && zikaron_store::layout::classify(&k.name) == zikaron_store::Kind::Entry)
        .map(|k| k.name.clone())
        .collect()
}

/// An entry's on-disk name under a names key.
pub fn entry_name_under(nk: &NameKey, id: &str) -> Result<EntryName, Trouble> {
    EntryName::parse(&nk.name(Logical::Entry(id))).ok_or_else(|| Trouble::new(Code::Ledger, id.to_string()))
}

/// The on-disk file name for a logical entry file name (`<id>.entry`) under a names key.
pub fn disk_entry_file(file: &str, nk: &NameKey) -> Result<String, Trouble> {
    let id = zikaron_store::layout::parse_entry_file(file).ok_or_else(|| Trouble::new(Code::Ledger, file.to_string()))?;
    Ok(zikaron_store::layout::entry_file_name(&entry_name_under(nk, id.as_str())?))
}

/// One home's ledger.
pub struct Ledger {
    dir: LedgerDir,
}

impl Ledger {
    /// Open a ledger directory (created when missing) whose entries are sealed.
    pub fn open_or_create(root: impl Into<PathBuf>) -> Result<Ledger, Trouble> {
        Ok(Ledger { dir: LedgerDir::open_or_create(root).map_err(store)? })
    }

    /// Open an existing ledger directory.
    pub fn open(root: impl Into<PathBuf>) -> Result<Ledger, Trouble> {
        Ok(Ledger { dir: LedgerDir::open(root).map_err(store)? })
    }

    pub fn root(&self) -> &Path {
        self.dir.root()
    }

    /// The underlying store directory (for layout reports and sweeps, which do not read entry bytes).
    pub fn store(&self) -> &LedgerDir {
        &self.dir
    }

    /// The home this ledger is in (the ledger directory's parent), which owns its entries.
    pub fn home_root(&self) -> PathBuf {
        self.root().parent().map(Path::to_path_buf).unwrap_or_else(|| self.root().to_path_buf())
    }

    /// What reading one of this ledger's entries expects (`file`: its on-disk name, when known).
    fn expect(owner: Option<Owner>, file: Option<&str>) -> Expect {
        Expect { owner, doc: Doc::Entry, rel: file.map(|f| format!("{LEDGER_DIR}/{f}")) }
    }

    /// Strict read: the decrypted audit pile. Anything the store cannot account for, any empty entry file
    /// ([`EMPTY`]), or any entry that does not open fails the whole read.
    pub fn pile(&self, k: &Keys) -> Result<Pile, Trouble> {
        let ex = Ledger::expect(k.owner.clone(), None);
        let p = self.dir.pile().map_err(store)?;
        let empty = self.empty_entries()?;
        if !empty.is_empty() {
            return Err(empty_trouble(&empty));
        }
        let mut items = Vec::with_capacity(p.items.len());
        for (i, b) in p.items.iter().enumerate() {
            items.push(open_with(k.local, &ex, b, &format!("{} #{i}", self.root().display()), Some(k.names))?);
        }
        Ok(Pile { items })
    }

    /// Zero-byte files under an entry's name in this ledger, sorted by name.
    pub fn empty_entries(&self) -> Result<Vec<String>, Trouble> {
        let root = self.root();
        let listing = std::fs::read_dir(root).map_err(|e| Trouble::io(&e, &root.display().to_string()))?;
        let mut out = Vec::new();
        for item in listing {
            let item = item.map_err(|e| Trouble::io(&e, &root.display().to_string()))?;
            let name = item.file_name().to_string_lossy().into_owned();
            if zikaron_store::layout::classify(&name) != zikaron_store::Kind::Entry {
                continue;
            }
            // `symlink_metadata`: a link is treated as itself, as the store does.
            if std::fs::symlink_metadata(item.path()).is_ok_and(|m| m.is_file() && m.len() == 0) {
                out.push(name);
            }
        }
        out.sort();
        Ok(out)
    }

    /// Lenient read: every entry that opens, plus each skipped file with its reason (entries that do not open are
    /// listed as unreadable).
    pub fn survey(&self, k: &Keys) -> Result<Survey, Trouble> {
        let ex = Ledger::expect(k.owner.clone(), None);
        let s = self.dir.survey().map_err(store)?;
        let mut out = Survey { items: Vec::with_capacity(s.items.len()), skipped: s.skipped };
        for (i, b) in s.items.iter().enumerate() {
            match open_with(k.local, &ex, b, "", Some(k.names)) {
                Ok(p) => out.items.push(p),
                Err(_) => zikaron_store::ledger::note_skip(&mut out.skipped, &format!("sealed #{i}"), zikaron_store::Why::Unreadable),
            }
        }
        out.skipped.sort_by(|a: &Skip, b: &Skip| a.name.cmp(&b.name));
        Ok(out)
    }

    /// Read and decrypt one entry by its logical file name (`<entry id>.entry`); the on-disk name is keyed.
    pub fn read_named(&self, file: &str, k: &Keys) -> Result<Vec<u8>, Trouble> {
        let disk = disk_entry_file(file, k.names)?;
        let b = self.dir.read_named(&disk).map_err(store)?;
        open_with(k.local, &Ledger::expect(k.owner.clone(), Some(&disk)), &b, file, Some(k.names))
    }

    /// Seal one entry for this ledger, owned by the home numbered `number` (for staging several before writing
    /// any). Canonical content is checked before sealing (`envelope::seal_checked`).
    pub fn seal_entry(&self, plain: &[u8], local: &LocalKey, number: &str) -> Result<Vec<u8>, Trouble> {
        let id = Ident { owner: Owner::Home(number.to_string()), doc: Doc::Entry, logical: crate::envelope::logical_rel(Doc::Entry, "", plain)? };
        seal_checked(local, &id, plain)
    }

    /// Append one entry (by its id) for the home numbered `number`. The store refuses overwrites and sees only
    /// opaque bytes, and each seal uses a fresh nonce, so equality is checked here on the decrypted bytes: the
    /// same entry again is a no-op, a different one under the same name is refused with nothing changed.
    pub fn append(&self, name: &EntryName, plain: &[u8], k: &Keys, number: &str) -> Result<Stored, Trouble> {
        let name = &entry_name_under(k.names, name.as_str())?;
        let file = zikaron_store::layout::entry_file_name(name);
        match self.dir.read_named(&file) {
            Ok(have) => {
                let opened = open_with(k.local, &Ledger::expect(k.owner.clone(), Some(&file)), &have, &file, Some(k.names))?;
                if zikaron_store::ledger::identical(&opened, plain) {
                    return Ok(Stored::AlreadyThere);
                }
                return Err(store(zikaron_store::Trouble::named(zikaron_store::Code::Conflict, &file)));
            }
            Err(t) if t.code == zikaron_store::Code::Absent => {}
            Err(t) => return Err(store(t)),
        }
        let sealed = self.seal_entry(plain, k.local, number)?;
        self.dir.append(name, &sealed).map_err(store)
    }
}
