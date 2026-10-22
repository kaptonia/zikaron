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

/// Switch seat. With an identity register, the register's write and the other seat's home opened and read run in
/// the background (`Kind::Seat`, as every long task of the product runs), and the shell moves into that seat where
/// it lands ([`seat_landed`]); the seat control shows the seat it goes to at once (`Shell::seat_wish`). A press
/// while a switch runs turns the wish round, and the last one stands: when the switch lands on another seat than
/// the one wished, it switches again. Without a register there is no identity; the seat follows settings, so
/// readings switch along and no old seat stays on screen.
pub(super) fn switch_role(shell: &mut Shell) -> Result<Applied, crate::fault::Fault> {
    let want = shell.seat_wish.unwrap_or(shell.settings.role).other();
    if shell.tasks.in_flight(Kind::Seat) {
        shell.seat_wish = Some(want);
        return Ok(Applied::Started(Kind::Seat));
    }
    if !shell.identity_listed() {
        shell.commit_settings(|s| s.role = s.role.other())?;
        shell.seat_identities(Some(crate::register::view()?));
        return Ok(Applied::Seated(shell.settings.role));
    }
    Ok(seat_start(shell, want))
}

/// A seat switch read in the background, for the shell to move into: the seat, its home entered on disk, the key
/// address that seat signs with (none while locked), and the identity table after the switch.
#[derive(Debug)]
pub struct Seating {
    seat: crate::roles::Role,
    entering: Entering,
    anchor: Option<crate::key::Address>,
    anchor_public: Option<[u8; 33]>,
    view: crate::identity::Registry,
    /// The identity switched to, when this is a switch of identity (`identity_start`); none for a switch of seat.
    identity: Option<String>,
}

/// Start switching to `want` in the background.
fn seat_start(shell: &mut Shell, want: crate::roles::Role) -> Applied {
    let ask = OpeningAsk::of(shell);
    let unlocked = shell.unlocked();
    shell.seat_wish = Some(want);
    match shell.tasks.spawn(Kind::Seat, move || {
        let Some((row, seat)) = crate::register::now_row_listed()? else {
            return Err(crate::fault::Fault::known(crate::fault::Known::NoIdentity, crate::places::registry_file()));
        };
        let id = row.id.clone();
        let row = crate::register::change(|reg| crate::identity::switch(reg, &id, want))?;
        // Anything failing after the register moved puts it back: the switch is had whole or not at all.
        let rest = (|| {
            let entering = entering(&row, want, &ask)?;
            let key = if unlocked { crate::key::load(crate::register::account_now()?.as_deref())? } else { None };
            let (anchor, anchor_public) = (key.as_ref().and_then(|s| s.address()), key.as_ref().and_then(|s| s.public()));
            drop(key);
            let view = crate::register::view()?;
            Ok(Seating { seat: want, entering, anchor, anchor_public, view, identity: None })
        })();
        match rest {
            Ok(s) => Ok(Done::Seated(crate::task::Seated::new(s))),
            Err(f) => {
                if seat != want {
                    put_back(&id, seat);
                }
                Err(f)
            }
        }
    }) {
        Spawned::Started => {
            // An answer of an earlier switch not yet taken is not this one's.
            shell.said.remove(&Kind::Seat);
            Applied::Started(Kind::Seat)
        }
        Spawned::InFlight => Applied::Refused(Kind::Seat),
    }
}

/// Switch identity (`Action::SwitchIdentity`), as a seat switch runs: the register's write and the identity's home
/// opened and read run in the background (`Kind::Seat`), and the shell moves into it where it lands
/// ([`seat_landed`]), on the seat in use if the identity occupies it, else its first occupied seat
/// (`Row::seat_here_or_first`). Asked while a switch runs, the identity is wished (`Shell::identity_wish`, the last
/// one asked stands; a seat wished before it gives way) and the switch goes on to it when the one running lands.
pub(super) fn switch_identity(shell: &mut Shell, id: &str) -> Applied {
    if shell.tasks.in_flight(Kind::Seat) {
        shell.identity_wish = Some(id.to_string());
        shell.seat_wish = None;
        return Applied::Started(Kind::Seat);
    }
    identity_start(shell, id.to_string())
}

/// Start switching to identity `id` in the background. Anything failing after the register moved puts it back to
/// the identity and seat it named before: the switch is had whole or not at all, and the shell never left.
fn identity_start(shell: &mut Shell, id: String) -> Applied {
    let ask = OpeningAsk::of(shell);
    let unlocked = shell.unlocked();
    let here = shell.settings.role;
    match shell.tasks.spawn(Kind::Seat, move || {
        let before = crate::register::now_row_listed()?.map(|(r, s)| (r.id, s));
        let seat = crate::register::view()?.find(&id).map(|r| r.seat_here_or_first(here)).unwrap_or(crate::roles::Role::Author);
        let row = crate::register::change(|reg| crate::identity::switch(reg, &id, seat))?;
        let rest = (|| {
            let entering = entering(&row, seat, &ask)?;
            let key = if unlocked { crate::key::load(crate::register::account_now()?.as_deref())? } else { None };
            let (anchor, anchor_public) = (key.as_ref().and_then(|s| s.address()), key.as_ref().and_then(|s| s.public()));
            drop(key);
            let view = crate::register::view()?;
            Ok(Seating { seat, entering, anchor, anchor_public, view, identity: Some(row.id.clone()) })
        })();
        match rest {
            Ok(s) => Ok(Done::Seated(crate::task::Seated::new(s))),
            Err(f) => {
                if let Some((was, was_seat)) = before.filter(|(was, was_seat)| *was != row.id || *was_seat != seat) {
                    put_back(&was, was_seat);
                }
                Err(f)
            }
        }
    }) {
        Spawned::Started => {
            // An answer of an earlier switch not yet taken is not this one's.
            shell.said.remove(&Kind::Seat);
            Applied::Started(Kind::Seat)
        }
        Spawned::InFlight => Applied::Refused(Kind::Seat),
    }
}

/// A switch of seat or of identity that failed after the register moved puts it back on the identity and seat it
/// named before: a switch is had whole or not at all, so the next start opens the home that was open, under the key
/// that signed there. The one place both switches take it back.
fn put_back(was: &str, was_seat: crate::roles::Role) {
    let _ = crate::register::change(|reg| crate::identity::switch(reg, was, was_seat));
}

/// Where a switch of seat or identity lands: the shell moves into the seat read, its key address and identity table
/// in hand; then goes on to the identity wished meanwhile if it is another, else to the seat wished meanwhile if it
/// is another. A refusal comes only from the background half, which put the register back before answering: it is
/// said as it is (the shell never left), and the seat control goes back to the seat the shell stands in. Moving the
/// shell in ([`entered`]) cannot fail, so a switch the register took lands whole.
pub(super) fn seat_landed(shell: &mut Shell, got: Result<Done, crate::fault::Fault>) -> Applied {
    let s = match got {
        Ok(Done::Seated(s)) => s.take(),
        Ok(_) => None,
        Err(f) => {
            shell.seat_wish = None;
            shell.identity_wish = None;
            return shell.trouble(f);
        }
    };
    let Some(Seating { seat, entering, anchor, anchor_public, view, identity }) = s else {
        shell.seat_wish = None;
        shell.identity_wish = None;
        return shell.trouble(crate::fault::Fault::known(crate::fault::Known::OutcomeLost, Kind::Seat.as_str().to_string()));
    };
    entered(shell, entering);
    let now_id = crate::identity::now_row(&view).map(|(r, _)| r.id);
    shell.anchor = anchor;
    shell.anchor_public = anchor_public;
    shell.seat_identities(Some(view));
    // Another identity was asked for meanwhile: this landing is a step on the way, said as the switch going on.
    if let Some(w) = shell.identity_wish.take().filter(|w| Some(w) != now_id.as_ref()) {
        return identity_start(shell, w);
    }
    match (shell.seat_wish.take(), identity) {
        // Another seat was wished meanwhile: likewise a step on the way.
        (Some(w), _) if w != seat => seat_start(shell, w),
        (_, Some(id)) => Applied::IdentitySwitched { id },
        (_, None) => Applied::Seated(seat),
    }
}
