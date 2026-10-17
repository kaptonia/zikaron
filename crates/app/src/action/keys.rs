use super::*;

/// Changes the time zone only if the change can be saved. If not (no home, read-only), it is refused by name:
/// a change shown but not saved would silently revert at the next start.
pub(super) fn set_zone(shell: &mut Shell, zone: crate::when::Zone) -> Result<crate::when::Zone, crate::fault::Fault> {
    shell.may_save_settings()?;
    shell.commit_settings(|s| s.zone = Some(zone))?;
    crate::when::set(zone);
    Ok(zone)
}

/// The language is a machine preference: the passcode gate and the first-run wizard are drawn before any home's
/// sealed settings can be read, so the choice is kept in this machine's settings first, and a change that cannot
/// be kept there is refused by name. An open home's settings take it too, since they speak first
/// (`Shell::speaks`): an open home that cannot be written (another window writes it) refuses the change, as a
/// change shown but not kept would come back at the next start. Without a home, the machine's choice is the one.
pub(super) fn set_lang(shell: &mut Shell, lang: crate::lang::Lang) -> Result<crate::lang::Lang, crate::fault::Fault> {
    if shell.home.is_some() {
        shell.may_save_settings()?;
    }
    shell.machine = crate::machine::update(|m| m.lang = Some(lang))?;
    if shell.home.is_some() {
        shell.commit_settings(|s| s.lang = Some(lang))?;
    }
    crate::lang::set(lang);
    Ok(lang)
}

/// The name of a seat, so a refusal can say which seat to switch to.
pub(super) fn seat_word(r: crate::roles::Role) -> crate::lang::Key {
    match r {
        crate::roles::Role::Author => crate::lang::Key::IdSeatAuthor,
        crate::roles::Role::Grantee => crate::lang::Key::IdSeatGrantee,
    }
}

/// The register's current identity and seat. Without a register (or with no current row) there is no
/// identity, so nothing that makes or takes a key goes on: refused by name, naming the register.
pub(super) fn listed_now() -> Result<(crate::identity::Row, crate::roles::Role), crate::fault::Fault> {
    crate::register::now_row_listed()?
        .ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoIdentity, crate::places::registry_file()))
}

/// Returns the signing key, only for the owner of the open home and only for a seat allowed this domain.
///
/// The key follows the register's current identity while the open home is chosen separately, so the two can
/// disagree (a home opened in settings that belongs to another identity, or a half-finished identity switch).
/// Signing then would append an entry with the wrong author, and the append-only ledger would read as broken
/// from then on. So with a register, the key is handed out only when the open home is the one registered for
/// the current identity's current seat; otherwise it is refused by name before anything is written. Without a
/// register there is no identity, and no key: refused by name.
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
    let (row, seat) = listed_now()?;
    if let Some(home) = shell.home.as_ref() {
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
    // The register's current identity has one slot per key: switching seat switches the derived key and that
    // seat's home.
    if let Some((row, seat)) = crate::register::now_row_listed()? {
        let (id, other) = (row.id.clone(), seat.other());
        let row = crate::register::change(|reg| crate::identity::switch(reg, &id, other))?;
        enter(shell, &row, other)?;
        return Ok(shell.settings.role);
    }
    shell.commit_settings(|s| s.role = s.role.other())?;
    // Without a register there is no identity; the seat follows settings, so readings switch along and no old
    // seat stays on screen.
    shell.seat_identities(Some(crate::register::view()?));
    Ok(shell.settings.role)
}
