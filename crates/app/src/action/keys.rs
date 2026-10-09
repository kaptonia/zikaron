use super::*;

pub(super) fn make_anchor_key(shell: &mut Shell) -> Result<Address, crate::fault::Fault> {
    let s = crate::key::generate()?;
    let a = s
        .address()
        .ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::KeyMalformed, crate::lang::t(crate::lang::Key::Tail013).to_string()))?;
    crate::key::install(crate::register::account_now()?.as_deref(), &s)?;
    // Load the key back and compare: the claim is that the key vault holds this key, so ask the vault rather
    // than trusting the store call's success or the freshly generated key.
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

/// Changes the time zone only if the change can be saved. If not (no home, read-only), it is refused by name:
/// a change shown but not saved would silently revert at the next start.
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
    // The passcode gate is shown before the home's sealed settings can be read, so the choice is also kept on
    // this machine. A failure here is reported; the gate just keeps its previous language.
    match crate::machine::update(|m| m.lang = Some(lang)) {
        Ok(m) => shell.machine = m,
        Err(f) => shell.faults.push(f),
    }
    Ok(lang)
}

/// The name of a seat, so a refusal can say which seat to switch to.
pub(super) fn seat_word(r: crate::roles::Role) -> crate::lang::Key {
    match r {
        crate::roles::Role::Author => crate::lang::Key::IdSeatAuthor,
        crate::roles::Role::Grantee => crate::lang::Key::IdSeatGrantee,
    }
}

/// Returns the signing key, only for the owner of the open home and only for a seat allowed this domain.
///
/// The key follows the register's current identity while the open home is chosen separately, so the two can
/// disagree (a home opened in settings that belongs to another identity, or a half-finished identity switch).
/// Signing then would append an entry with the wrong author, and the append-only ledger would read as broken
/// from then on. So with a register, the key is handed out only when the open home is the one registered for
/// the current identity's current seat; otherwise it is refused by name before anything is written. Without a
/// register (machines not yet upgraded, a test's temporary home) the key at the account base is used.
pub(super) fn signing_key(shell: &Shell, u: crate::sign::Use) -> Result<crate::key::Secret, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    // Check seat × domain against the closed table first: only `sign::seat_may` decides, and the refusal
    // names the seat to switch to (`sign::seat_for`).
    let seat = shell.settings.role;
    if !crate::sign::seat_may(seat, u) {
        let go = crate::sign::seat_for(u).unwrap_or(seat.other());
        return Err(Fault::known(
            Known::SeatDomain,
            crate::lang::filln(crate::lang::Key::TailSeatDomain, &[u.as_str(), crate::lang::t(seat_word(go))]),
        ));
    }
    if let (Some((row, seat)), Some(home)) = (crate::register::now_row_listed()?, shell.home.as_ref()) {
        // An empty seat has no home, so no home belongs to it: a mismatch is refused by name before anything
        // is written.
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
    // The register's current identity has one slot per key: switching seat switches the derived key and that
    // seat's home.
    if let Some((row, seat)) = crate::register::now_row_listed()? {
        {
            let (id, slot, other) = (row.id.clone(), row.slot(), seat.other());
            if slot == Slot::Own {
                let row = crate::register::change(other, |reg| crate::identity::switch(reg, &id, other))?;
                enter(shell, &row, other)?;
                return Ok(shell.settings.role);
            }
            // An existing-key slot: only the view changes, and the register's current seat follows.
            shell.commit_settings(|s| s.role = s.role.other())?;
            let seat = shell.settings.role;
            crate::register::change(seat, |reg| crate::identity::switch(reg, &id, seat))?;
            shell.seat_identities(Some(crate::register::view(seat)?));
            return Ok(shell.settings.role);
        }
    }
    shell.commit_settings(|s| s.role = s.role.other())?;
    // Without a register the identity reads as the existing slot key and the current seat follows settings,
    // so readings switch along and no old seat stays on screen.
    shell.seat_identities(Some(crate::register::view(shell.settings.role)?));
    Ok(shell.settings.role)
}
