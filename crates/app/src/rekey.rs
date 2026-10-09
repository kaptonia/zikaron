//! Making another identity primary: a new master key, with every key slot and local file resealed.
//!
//! The orchestration lives here, next to whole-machine backup and restore, which reseal everything the same
//! way. The key store keeps only the key core (the master key, what derives from it, the recovery seal), so it
//! never depends on local data.
//!
//! Only the primary identity's recovery seal opens the vault, so changing the primary also changes the master
//! key M. Otherwise anyone holding the old primary's words and a copy of the old vault file could open the old
//! M through the old recovery seal and read local data with the key derived from it; with a new M, the old
//! words open nothing written afterwards.
//!
//! The change is all or nothing: a new M sealed by the passcode; one recovery seal for the new primary; every
//! key slot resealed under M (the old primary stays as a secondary identity); every local file resealed under
//! the new local data key and renamed under the new names key, so nothing named under the old key remains.
//! Everything is staged beside its target first (`<file>.zk-next`), the new vault last. Renaming the vault is
//! the commit point; staged files settle right after. An interruption before the commit leaves the old vault,
//! passcode and data intact; one after it is completed at the next unlock (`local::settle_pending`).

use crate::fault::{Fault, Known};

/// Make `id` the primary identity. The passcode `pin` has just opened the vault (checked by the caller) and
/// seals the new master key. An imported-key identity must have exported its key file first
/// (`PRIMARY_NO_KEYFILE`); otherwise a forgotten passcode would have no way back.
pub fn set_primary(id: &str, pin: &str) -> Result<(), Fault> {
    crate::trace::mark(crate::feature::Feature::H4);
    let reg = crate::register::read()?.ok_or_else(|| Fault::known(Known::NoIdentity, id.to_string()))?;
    let row = reg.find(id).ok_or_else(|| Fault::known(Known::NoIdentity, id.to_string()))?.clone();
    if let Some((p, _)) = crate::keybox::primary()? {
        if p.eq_ignore_ascii_case(&row.id) {
            return Err(Fault::known(Known::PrimaryAlready, row.id.clone()));
        }
    }
    let (kind, secret_slot) = match row.kind() {
        crate::identity::Kind::Words => (crate::keybox::PrimaryKind::Words, crate::places::seed_slot(&row.id)),
        crate::identity::Kind::Existing => {
            if !row.backed_file {
                return Err(Fault::known(Known::PrimaryNoKeyFile, row.id.clone()));
            }
            let acct = row.accounts().into_iter().next().ok_or_else(|| Fault::known(Known::KeychainMissing, row.id.clone()))?;
            (crate::keybox::PrimaryKind::KeyFile, acct)
        }
    };
    let mut secret = crate::keybox::get(&secret_slot)?.ok_or_else(|| Fault::known(Known::KeychainMissing, secret_slot.clone()))?;
    // Every slot, opened under the old master key, to be resealed under the new one.
    let mut slots: Vec<(String, Vec<u8>)> = Vec::new();
    for a in crate::keybox::accounts()? {
        if let Some(b) = crate::keybox::get(&a)? {
            slots.push((a, b));
        }
    }
    let nb = crate::keybox::build_new(crate::keybox::PinFor::New(pin), Some((&row.id, kind, &secret)), &slots);
    zikaron_ui::secret::wipe(&mut secret);
    for (_, b) in slots.iter_mut() {
        zikaron_ui::secret::wipe(b);
    }
    let nb = nb?;
    let new = nb.local_key();
    let old = crate::keybox::local_key()?;
    // The staged vault is written first. While it is only staged the change has not taken effect, whatever else
    // is staged (`local::settle_pending`), so an interruption before the rename leaves the old state intact.
    let staged = (|| -> Result<(), Fault> {
        crate::keybox::stage_new(&nb)?;
        // The names key changes with the master key, so every file is resealed and staged under its new name;
        // the old names are removed once the change takes effect (`local::restage`).
        crate::local::restage(&old, &new, &nb.name_key())?;
        Ok(())
    })();
    let undo = || {
        // Nothing took effect: drop every staged file; the vault and data stay as they were.
        let _ = crate::keybox::drop_staged();
        let _ = crate::local::drop_change();
    };
    if let Err(f) = staged {
        undo();
        return Err(f);
    }
    crate::local::cut_point(crate::local::Cut::BeforeCommit)?;
    if let Err(f) = crate::keybox::commit_new(nb) {
        undo();
        return Err(f);
    }
    crate::local::cut_point(crate::local::Cut::AfterCommit)?;
    // The change has taken effect; a staged file that cannot be moved into place now settles at the next unlock.
    if let Err(f) = crate::local::settle_pending() {
        crate::local::note_trouble(f);
    }
    Ok(())
}
