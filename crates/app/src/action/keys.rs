use super::*;

pub(super) fn listed_now() -> Result<(crate::identity::Row, crate::roles::Role), crate::fault::Fault> {
    crate::register::now_row_listed()?
        .ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoIdentity, crate::places::registry_file()))
}

pub(super) fn signing_key(shell: &Shell, u: crate::sign::Use) -> Result<crate::key::Secret, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    seat_checked(shell, u)?;
    let (row, seat) = listed_now()?;
    if let Some(home) = shell.home.as_ref() {
        let owns = row
            .home(seat)
            .map(|h| crate::home::same_place(&h, home.root()))
            .unwrap_or(false);
        if !owns {
            return Err(Fault::known(Known::NoIdentity, home.root().display().to_string()));
        }
    }
    crate::key::load(crate::register::account_now()?.as_deref())?.ok_or_else(|| Fault::known(Known::KeychainMissing, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::SetNoKey).to_string())))
}

fn seat_checked(shell: &Shell, u: crate::sign::Use) -> Result<(), crate::fault::Fault> {
    let seat = shell.settings.role;
    if !crate::sign::seat_may(seat, u) {
        let go = crate::sign::seat_for(u).unwrap_or(seat.other());
        return Err(crate::fault::Fault::known(
            crate::fault::Known::SeatDomain,
            crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::TailSeatDomain, &[u.as_str(), crate::lang::t(crate::roles::seat_word(go))])),
        ));
    }
    Ok(())
}

pub(super) fn consent_key(shell: &Shell, address: &Address) -> Result<Option<crate::key::Secret>, crate::fault::Fault> {
    seat_checked(shell, crate::sign::Use::Sign(crate::sign::Face::Consent))?;
    let reg = crate::register::view()?;
    let held = reg.rows.iter().any(|r| r.seats().into_iter().any(|s| r.current(s) == Some(*address)));
    if !held {
        return Ok(None);
    }
    crate::key::load(Some(&crate::places::key_slot(address)))
}

pub(super) struct RerootCarry(crate::facex::State, Option<crate::facex::Pair>, Option<u64>, Option<FeeReading>);

pub(super) fn reroot_carry(shell: &Shell) -> RerootCarry {
    RerootCarry(shell.persona.clone(), shell.face_pair.clone(), shell.settings.chain_id, shell.reroot_fee.clone())
}

pub(super) fn reroot_carried(shell: &mut Shell, carried: RerootCarry, now_id: Option<&str>) {
    let carried = (carried.0, carried.1, carried.2, carried.3);
    if let (crate::facex::State::Read(was), Some(pair)) = (&carried.0, &carried.1) {
        if pair.other_id.is_some() && pair.other_id.as_deref() == now_id && matches!(pair.state, crate::facex::State::Read(_)) && matches!(shell.persona, crate::facex::State::Unread) {
            let back = crate::key::Address::parse(&was.table.root);
            let side = match pair.side {
                crate::facex::Side::Old => crate::facex::Side::New,
                crate::facex::Side::New => crate::facex::Side::Old,
            };
            let was_id = shell.identities.as_ref().and_then(|reg| back.and_then(|b| reg.rows.iter().find(|r| r.address(crate::roles::Role::Author) == Some(b)).map(|r| r.id.clone())));
            if let Some(back) = back {
                shell.persona = pair.state.clone();
                shell.face_pair = Some(crate::facex::Pair { side, other: back, other_id: was_id, other_chain: carried.2, state: carried.0.clone() });
                shell.reroot_fee = carried.3.clone();
            }
        }
    }
}
