//! Making another identity primary: a new master key, every key and local file resealed.
//!
//! The orchestration of a master key change lives with the whole-machine backup and restore, which reseals
//! everything the same way; the key store keeps only the key core: the master key, what derives from it and
//! the recovery seal. So the key store never depends on local data.
//!
//! The primary identity is the only one whose recovery seal opens the vault. Changing it changes the master
//! key M in the same step: whoever holds the old primary's words and a copy of the old vault file could open
//! the old M through the old recovery seal, and with the local data key derived from M, read local data; with
//! a new M, the old words open nothing written from then on.
//!
//! One change, whole or not at all: a new M; the passcode seals it; the new primary gets the one recovery seal;
//! every key slot is resealed under it (the old primary stays, as a secondary identity); every local file is
//! resealed under the new local data key and renamed under the new names key (every name on disk is keyed by
//! it: nothing named under the old one is left). Everything is staged beside its place first (`<file>.zk-next`), the
//! new vault last; the vault's rename is the moment it takes effect, and the staged files settle right after.
//! A cut before that moment leaves the old vault, the old passcode and the old data as they were; a cut after
//! it settles on the next unlock (`local::settle_pending`).

use crate::fault::{Fault, Known};

/// Make `id` the primary identity. The passcode `pin` has just opened the vault (the caller's gate); it seals
/// the new master key. An imported-key identity must have exported its key file first
/// (`PRIMARY_NO_KEYFILE`): without it, a forgotten passcode would have no way back.
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
    // Every slot, opened under the old master key, to be sealed under the new one.
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
    // The staged vault goes down first: while it is on disk the change has not taken effect, whatever else is
    // staged (`local::settle_pending`), so a cut anywhere before the rename leaves the old state whole.
    let staged = (|| -> Result<(), Fault> {
        crate::keybox::stage_new(&nb)?;
        // Every file is resealed and, the names key changing with the master key, renamed: each is staged at
        // its new name, the old names going once the change takes effect (`local::restage`).
        crate::local::restage(&old, &new, &nb.name_key())?;
        Ok(())
    })();
    let undo = || {
        // Nothing took effect: every staged file goes, the vault and the data stay as they were.
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
    // The change has taken effect; a staged file that cannot take its place now does so at the next unlock.
    if let Err(f) = crate::local::settle_pending() {
        crate::local::note_trouble(f);
    }
    Ok(())
}
