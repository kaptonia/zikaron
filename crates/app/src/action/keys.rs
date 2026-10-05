use super::*;

pub(super) fn make_anchor_key(shell: &mut Shell) -> Result<Address, crate::fault::Fault> {
    let s = crate::key::generate()?;
    let a = s
        .address()
        .ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::KeyMalformed, crate::lang::t(crate::lang::Key::Tail013).to_string()))?;
    crate::key::install(crate::register::account_now()?.as_deref(), &s)?;
    // Load it back and compare once after storing. The system saying it accepted it is the system's word,
    // while this step claims a state ("the key vault has this key"); the owner of that state is the key
    // vault, so ask it, never substituting the freshly generated key.
    match crate::key::load(crate::register::account_now()?.as_deref())? {
        Some(back) if back.address() == Some(a) => {}
        _ => {
            return Err(crate::fault::Fault::known(
                crate::fault::Known::KeyNotStored,
                crate::places::key_account().to_string(),
            ))
        }
    }
    shell.anchor = Some(a);
    Ok(a)
}

/// Before switching, ask whether it can be recorded, then switch. If it cannot be recorded (no home,
/// read-only), refuse by name and do not switch: switched on the face but not on disk would revert at next
/// startup, a silent failure.
pub(super) fn set_zone(shell: &mut Shell, zone: crate::when::Zone) -> Result<crate::when::Zone, crate::fault::Fault> {
    shell.may_save_settings()?;
    shell.commit_settings(|s| s.zone = Some(zone))?;
    crate::when::set(zone);
    Ok(zone)
}

pub(super) fn set_lang(shell: &mut Shell, lang: crate::lang::Lang) -> Result<crate::lang::Lang, crate::fault::Fault> {
    shell.may_save_settings()?;
    shell.commit_settings(|s| s.lang = Some(lang))?;
    crate::lang::set(lang);
    // The passcode gate speaks before the home's (sealed) settings can be read: the last choice is also kept
    // on this machine. Not keeping it only leaves the gate in the language it had; it is said, not hidden.
    let mut m = crate::machine::read()?;
    m.lang = Some(lang);
    match crate::machine::write(&m) {
        Ok(()) => shell.machine = m,
        Err(f) => shell.faults.push(f),
    }
    Ok(lang)
}

/// A seat's name (a refusal must say which seat to go to; the window side has a same-named place).
pub(super) fn seat_word(r: crate::roles::Role) -> crate::lang::Key {
    match r {
        crate::roles::Role::Author => crate::lang::Key::IdSeatAuthor,
        crate::roles::Role::Grantee => crate::lang::Key::IdSeatGrantee,
    }
}

/// The signing key is handed only to the owner of the open home, and only to a seat that may use this domain.
///
/// With the signing key decided by the register's "current identity" and the open home by "open home", the
/// two could disagree: opening a home in settings that does not belong to the current identity, or a
/// half-finished identity switch (register switched, home not), would sign the next entry into this ledger
/// with another key; the core would read an author mismatch, and since the ledger is append-only, the
/// ledger would be judged broken from then on (the victim: whoever writes this ledger). So with a register,
/// the key is handed out only when the open home is the one registered for the current identity's current
/// seat; otherwise refused by name, writing not one byte. Without a register (machines before upgrade, a
/// test's temporary home) the key at the account base is taken as before.
pub(super) fn signing_key(shell: &Shell, u: crate::sign::Use) -> Result<crate::key::Secret, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    // Seat × domain passes the closed table first: this use is named by the caller, and allowing or refusing
    // is decided only in `sign::seat_may`; the refusal can say which seat to go to (`sign::seat_for`).
    let seat = shell.settings.role;
    if !crate::sign::seat_may(seat, u) {
        let go = crate::sign::seat_for(u).unwrap_or(seat.other());
        return Err(Fault::known(
            Known::SeatDomain,
            crate::lang::filln(crate::lang::Key::TailSeatDomain, &[u.as_str(), crate::lang::t(seat_word(go))]),
        ));
    }
    if let (Some((row, seat)), Some(home)) = (crate::register::now_row_listed()?, shell.home.as_ref()) {
        // An empty seat has no home, so no home "belongs to it": a mismatch is refused by name, writing not
        // one byte.
        let owns = row
            .home(seat)
            .map(|h| crate::home::same_place(&h, home.root()))
            .unwrap_or(false);
        if !owns {
            return Err(Fault::known(Known::NoIdentity, home.root().display().to_string()));
        }
    }
    crate::key::load(crate::register::account_now()?.as_deref())?.ok_or_else(|| Fault::known(Known::KeychainMissing, crate::lang::t(crate::lang::Key::SetNoKey).to_string()))
}

pub(super) fn switch_role(shell: &mut Shell) -> Result<crate::roles::Role, crate::fault::Fault> {
    use crate::identity::Slot;
    // The register's current identity has one slot per key by address: switching seat switches the derived
    // key and that seat's home.
    if let Some((row, seat)) = crate::register::now_row_listed()? {
        {
            let (id, slot, other) = (row.id.clone(), row.slot(), seat.other());
            if slot == Slot::Own {
                let row = crate::register::change(other, |reg| crate::identity::switch(reg, &id, other))?;
                enter(shell, &row, other)?;
                return Ok(shell.settings.role);
            }
            // The existing slot key: what changes is the view, and the register's current seat follows.
            shell.commit_settings(|s| s.role = s.role.other())?;
            let seat = shell.settings.role;
            crate::register::change(seat, |reg| crate::identity::switch(reg, &id, seat))?;
            shell.seat_identities(Some(crate::register::view(seat)?));
            return Ok(shell.settings.role);
        }
    }
    shell.commit_settings(|s| s.role = s.role.other())?;
    // Without a register the identity reads as the existing slot key and the current seat follows settings:
    // readings switch along, and the face keeps no old seat.
    shell.seat_identities(Some(crate::register::view(shell.settings.role)?));
    Ok(shell.settings.role)
}
