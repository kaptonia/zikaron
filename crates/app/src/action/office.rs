use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Back {
    Was,
    To(Option<(String, crate::roles::Role)>),
    Gone,
}

pub fn settle(shell: &mut Shell, write: impl FnOnce() -> Result<crate::identity::Registry, crate::fault::Fault>) -> Result<(), crate::fault::Fault> {
    let reg = write()?;
    shell.seat_identities(Some(reg));
    if let Err(f) = shell.refresh_anchor() {
        shell.faults.push(f);
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Followed {
    Same,
    Office,
    Moved,
    Failed(crate::fault::Fault),
}

pub fn follow_register(shell: &mut Shell) -> Followed {
    if !shell.unlocked() {
        return Followed::Same;
    }
    let listed = match crate::register::view() {
        Ok(v) => v,
        Err(f) => {
            shell.faults.push(f.clone());
            return Followed::Failed(f);
        }
    };
    let Some((row, seat)) = crate::identity::now_row(&listed) else { return Followed::Same };
    let held = shell.identities.as_ref().and_then(crate::identity::now_row);
    match held {
        Some((h, s)) if h.id.eq_ignore_ascii_case(&row.id) && s == seat => {
            if h == row && shell.anchor == row.current(seat) {
                return Followed::Same;
            }
            shell.seat_identities(Some(listed));
            if let Err(f) = shell.refresh_anchor() {
                shell.faults.push(f.clone());
                return Followed::Failed(f);
            }
            Followed::Office
        }
        _ => match entering(&row, seat, &OpeningAsk::of(shell)) {
            Ok(e) => {
                entered(shell, e);
                if let Err(f) = shell.refresh_anchor() {
                    shell.faults.push(f);
                }
                shell.seat_identities(Some(listed));
                Followed::Moved
            }
            Err(f) => {
                shell.faults.push(f.clone());
                Followed::Failed(f)
            }
        },
    }
}

pub(super) fn enter(shell: &mut Shell, row: &crate::identity::Row, seat: crate::roles::Role, back: Back) -> Result<(), crate::fault::Fault> {
    let (was, view) = crate::register::change(|reg| {
        let was = reg.current.clone();
        crate::identity::switch(reg, &row.id, seat)?;
        Ok((was, reg.clone()))
    })?;
    let e = match entering(row, seat, &OpeningAsk::of(shell)) {
        Ok(e) => e,
        Err(f) => {
            let to = match back {
                Back::Was => Some(was),
                Back::To(to) => Some(to),
                Back::Gone => None,
            };
            match to {
                Some(to) => {
                    if let Err(back) = crate::register::change(|reg| {
                        reg.current = to;
                        Ok(())
                    }) {
                        shell.faults.push(back);
                    }
                }
                None => {
                    shell.close_home();
                    shell.settings.role = seat;
                    if let Err(g) = shell.refresh_anchor() {
                        shell.faults.push(g);
                    }
                    match crate::register::view() {
                        Ok(v) => shell.seat_identities(Some(v)),
                        Err(g) => shell.faults.push(g),
                    }
                }
            }
            return Err(f);
        }
    };
    shell.seat_identities(Some(view));
    entered(shell, e);
    shell.refresh_anchor()?;
    Ok(())
}

#[derive(Debug)]
pub struct Entering {
    seat: crate::roles::Role,
    opening: Option<Opening>,
}

pub(super) fn entering(row: &crate::identity::Row, seat: crate::roles::Role, ask: &OpeningAsk) -> Result<Entering, crate::fault::Fault> {
    let home = row.home(seat);
    let mut opened_at = None;
    if let Some(root) = home.as_ref() {
        let mut o = opening(&root.display().to_string(), false, ask)?;
        o.seat_written(seat)?;
        opened_at = Some(o);
    }
    Ok(Entering { seat, opening: opened_at })
}

pub(super) fn entered(shell: &mut Shell, e: Entering) {
    let Entering { seat, opening } = e;
    let homed = opening.is_some();
    if let Some(o) = opening {
        opened(shell, o);
    }
    if !homed {
        shell.close_home();
        shell.settings.role = seat;
    }
}

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

#[derive(Debug)]
pub struct Seating {
    seat: crate::roles::Role,
    entering: Entering,
    anchor: Option<crate::key::Address>,
    anchor_public: Option<[u8; 33]>,
    view: crate::identity::Registry,
    identity: Option<String>,
}

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
            shell.said.remove(&Kind::Seat);
            Applied::Started(Kind::Seat)
        }
        Spawned::InFlight => Applied::Refused(Kind::Seat),
    }
}

pub(super) fn switch_identity(shell: &mut Shell, id: &str) -> Applied {
    if shell.tasks.in_flight(Kind::Seat) {
        shell.identity_wish = Some(id.to_string());
        shell.seat_wish = None;
        return Applied::Started(Kind::Seat);
    }
    let current = shell.identities.as_ref().and_then(crate::identity::now_row).map(|(x, _)| x.id);
    if current.is_some_and(|c| c.eq_ignore_ascii_case(id)) {
        return Applied::IdentityKept { id: id.to_string() };
    }
    identity_start(shell, id.to_string())
}

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
            shell.said.remove(&Kind::Seat);
            Applied::Started(Kind::Seat)
        }
        Spawned::InFlight => Applied::Refused(Kind::Seat),
    }
}

fn put_back(was: &str, was_seat: crate::roles::Role) {
    let _ = crate::register::change(|reg| crate::identity::switch(reg, was, was_seat));
}

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
    let carried = super::keys::reroot_carry(shell);
    let now_id = crate::identity::now_row(&view).map(|(r, _)| r.id);
    shell.seat_identities(Some(view));
    entered(shell, entering);
    super::keys::reroot_carried(shell, carried, now_id.as_deref());
    shell.anchor = anchor;
    shell.anchor_public = anchor_public;
    if let Some(w) = shell.identity_wish.take().filter(|w| Some(w) != now_id.as_ref()) {
        return identity_start(shell, w);
    }
    match (shell.seat_wish.take(), identity) {
        (Some(w), _) if w != seat => seat_start(shell, w),
        (_, Some(id)) => Applied::IdentitySwitched { id },
        (_, None) => Applied::Seated(seat),
    }
}

