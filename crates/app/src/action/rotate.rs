use super::*;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RotateStep {
    Look,
    Vet { to: String },
    Hand { to: String },
    Begin { source: crate::rotatex::Source, answers: Vec<(usize, crate::secret::Secret)> },
    Declare { chain: u64 },
    Write,
    Accede { chain: u64 },
    Sweep { chain: u64, value: u128 },
    Leave { chain: u64 },
    SweepTo { to: String },
    Cancel,
    Withdraw { chain: u64 },
    Enroll { chain: u64 },
}

impl RotateStep {
    pub fn writes(&self) -> bool {
        matches!(self, RotateStep::Write)
    }

    pub fn uses_key(&self) -> bool {
        !matches!(self, RotateStep::Look | RotateStep::Vet { .. } | RotateStep::SweepTo { .. })
    }
}

struct Seat {
    row: crate::identity::Row,
    seat: crate::roles::Role,
    root: Address,
    current: Address,
    next: Option<Address>,
    generation: u64,
}

fn seat_now() -> Result<Seat, crate::fault::Fault> {
    let (row, seat) = listed_now()?;
    seat_in(row, seat)
}

fn seat_in(row: crate::identity::Row, seat: crate::roles::Role) -> Result<Seat, crate::fault::Fault> {
    let office = row.office(seat).ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::SeatUnseated, row.id.clone()))?;
    let root = row.address(seat).ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::SeatUnseated, row.id.clone()))?;
    Ok(Seat { root, current: Address(office.current), next: office.next.map(Address), generation: office.generation, row, seat })
}

pub fn last_move(shell: &Shell, current: &Address) -> Option<(String, String)> {
    let rows = shell.rows.as_ref().map(|(r, _)| r.as_slice())?;
    rows.iter()
        .filter(|r| r.law == zikaron_glue::book::Law::Two && r.kind.goes_alone() && r.facts.to.as_deref().is_some_and(|to| to.eq_ignore_ascii_case(&current.hex())) && !r.author.eq_ignore_ascii_case(&current.hex()))
        .max_by_key(|r| r.seq)
        .map(|r| (r.id.clone(), r.author.clone()))
}

fn kept_local(shell: &Shell) -> std::collections::BTreeSet<String> {
    shell.rows.as_ref().map(|(r, _)| r.iter().filter(|x| x.lamp.stays_off()).map(|x| x.id.to_ascii_lowercase()).collect()).unwrap_or_default()
}

pub fn ledger_root(shell: &Shell) -> Option<Address> {
    let items = shell.home.as_ref()?.ledger().ok()?.pile().ok()?.items;
    zikaron_glue::book::root_of(&items).ok().and_then(|r| Address::parse(&r))
}

pub fn ledger_root_read(shell: &Shell) -> Option<Address> {
    let key = (shell.rows_at, shell.home.as_ref().map(|h| h.root().to_path_buf()));
    shell.faces_read.get(&key).or_else(|| shell.faces_read.held()).and_then(|l| l.root).map(Address)
}

pub(crate) fn enrolment_read(mine: &[crate::chainx::Endpoint], chain: u64, at: &[u8; 20], root: &Address, office: &Address) -> Result<(zikaron_send::book::Held, Result<crate::rotatex::Enrol, crate::rotatex::Closed>), crate::fault::Fault> {
    let held = zikaron_send::book::held(mine, chain, at, &root.0)?;
    let pending = zikaron_send::book::address(mine, chain, at, zikaron_anchor::book::Call::Pending, &root.0)?;
    let entered = zikaron_send::book::entered(mine, chain, at, &office.0)?;
    let plan = crate::rotatex::enrolment(held.root, held.current, pending, entered, &root.0, &office.0);
    Ok((held, plan))
}

pub(crate) fn closed_fault(chain: u64, root: &Address, office: &Address, c: crate::rotatex::Closed) -> crate::fault::Fault {
    use crate::fault::{Fault, Known};
    match c {
        crate::rotatex::Closed::RootElsewhere(under) => Fault::known(Known::BookClosed, format!("{chain} rootElsewhere {}", zikaron::hexfmt::encode(&under))),
        crate::rotatex::Closed::OfficeEntered => Fault::known(Known::BookClosed, format!("{chain} officeEntered {}", office.hex())),
        crate::rotatex::Closed::OtherKey(k) => Fault::known(Known::OfficeElsewhere, format!("{chain} root={} office={} book={}", root.hex(), office.hex(), zikaron::hexfmt::encode(&k))),
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LedgerFaces {
    pub root: Option<[u8; 20]>,
    pub faces: Vec<zikaron_glue::facegate::LedgerFace>,
}

pub(super) fn faces_in(items: &[Vec<u8>]) -> LedgerFaces {
    LedgerFaces { root: zikaron_glue::book::root_of(items).ok().and_then(|r| Address::parse(&r)).map(|a| a.0), faces: super::face::face_entries(items) }
}

fn faces_of(shell: &Shell) -> crate::rotatex::Faces {
    let key = (shell.rows_at, shell.home.as_ref().map(|h| h.root().to_path_buf()));
    let ledger = shell.faces_read.get(&key).or_else(|| shell.faces_read.held()).unwrap_or_default();
    let (home, sealed) = match &shell.persona {
        crate::facex::State::Read(b) => (b.table.home, zikaron_glue::facegate::sealed(&b.table).is_err()),
        _ => (None, false),
    };
    crate::rotatex::Faces { root: ledger.root, unseated: super::face::unseated_of(&ledger.faces, &shell.persona), home, sealed }
}

pub fn seat_before(shell: &Shell) -> Option<Address> {
    let (row, seat) = crate::identity::now_row(shell.identities.as_ref()?)?;
    let s = seat_in(row, seat).ok()?;
    Some(crate::rotatex::holder(s.current, s.next, None, moved_to(shell, &s.current).as_ref()))
}

pub fn rotation_stage(shell: &Shell) -> Option<crate::rotatex::Stage> {
    let (row, seat) = crate::identity::now_row(shell.identities.as_ref()?)?;
    let s = seat_in(row, seat).ok()?;
    let report = shell.audit_report_in_step();
    let basis: Vec<u64> = shell.basis_chains().into_iter().map(|(c, _, _)| c).collect();
    let chains = shell.rotation.clone().unwrap_or_default();
    let on = shell.on_chain();
    let queued = queued_ids(shell);
    if let Some(to) = shell.handed.as_deref().and_then(Address::parse) {
        let (id, by) = moved_to(shell, &to).unwrap_or_default();
        let at = crate::rotatex::stage_handed(&id, &by, report, &on, &basis);
        if at == crate::rotatex::Stage::Done {
            if let Some((id, old, _)) = sweep_from(shell, &s) {
                let left = left_for(shell, &old);
                if let Some(sweep) = crate::rotatex::sweep(&basis, &left, &crate::rotatex::settled_of(&shell.settings.swept, &id), None) {
                    return Some(sweep);
                }
            }
        }
        return Some(at);
    }
    if let Some(to) = shell.rot_given {
        let owing = crate::rotatex::Owing { on: &on, queued: &queued, own: None };
        return Some(crate::rotatex::stage_given(s.current, to, report, &owing, &kept_local(shell), &basis, &chains, &faces_of(shell)));
    }
    let moved = moved_to(shell, &s.current).map(|(id, _)| id);
    let owing = crate::rotatex::Owing { on: &on, queued: &queued, own: moved.as_deref() };
    let faces = faces_of(shell);
    let at = crate::rotatex::stage(s.current, s.next, moved.as_deref(), report, &owing, &kept_local(shell), &basis, &chains, &faces);
    if s.next.is_none() {
        let before = crate::rotatex::holder(s.current, s.next, None, moved_to(shell, &s.current).as_ref());
        if let Some(st) = crate::rotatex::standing(&chains, &basis, &ledger_root_read(shell).unwrap_or(s.root).0, &s.current.0, &before.0) {
            return Some(st);
        }
    }
    if let Some((id, old, _)) = sweep_from(shell, &s) {
        use crate::rotatex::Stage;
        let then = match &at {
            Stage::Done => Some(None),
            Stage::Anchoring { id: own } if !crate::rotatex::succession_out(own, shell.tasks.in_flight(Kind::Anchor), &out_ids(shell)) => Some(Some(&at)),
            Stage::Accede { chain } if shell.rot_sweep_out.is_some() || (faces.home != Some(*chain) && !crate::rotatex::step_waits(&at, shell.rot_sending)) => Some(Some(&at)),
            _ => None,
        };
        if let Some(then) = then {
            let left = left_for(shell, &old);
            if let Some(sweep) = crate::rotatex::sweep(&basis, &left, &crate::rotatex::settled_of(&shell.settings.swept, &id), then) {
                return Some(sweep);
            }
        }
    }
    Some(at)
}

fn sweep_from(shell: &Shell, s: &Seat) -> Option<(String, Address, Address)> {
    if s.next.is_some() || shell.rot_given.is_some() {
        return None;
    }
    if let Some(next) = shell.handed.as_deref().and_then(Address::parse) {
        let (id, by) = moved_to(shell, &next)?;
        let basis: Vec<u64> = shell.basis_chains().into_iter().map(|(c, _, _)| c).collect();
        let on = crate::rotatex::stage_handed(&id, &by, shell.audit_report_in_step(), &shell.on_chain(), &basis) == crate::rotatex::Stage::Done;
        let by = Address::parse(&by)?;
        return (on && by == s.current).then(|| (id, by, shell.rot_sweep_to.unwrap_or(next)));
    }
    let (id, by) = moved_to(shell, &s.current)?;
    let by = Address::parse(&by)?;
    let own = by == s.root || s.row.office(s.seat).is_some_and(|o| o.former.contains(&by.0));
    (own && by != s.current).then_some((id, by, s.current))
}

fn left_for(shell: &Shell, old: &Address) -> Vec<crate::rotatex::Left> {
    match (&shell.rot_left, &shell.rot_left_of) {
        (Some(l), Some(o)) if o == old => l.clone(),
        _ => Vec::new(),
    }
}

pub fn sweep_key(shell: &Shell) -> Option<(String, Address)> {
    let (row, seat) = shell.identities.as_ref().and_then(crate::identity::now_row)?;
    let s = seat_in(row, seat).ok()?;
    sweep_from(shell, &s).map(|(id, old, _)| (id, old))
}

fn fresh_of(eps: &[crate::chainx::Endpoint], basis: &[(u64, Address)], who: Address) -> Vec<(u64, u128)> {
    basis
        .iter()
        .filter_map(|(chain, _)| {
            let mine: Vec<crate::chainx::Endpoint> = eps.iter().filter(|e| e.chain == *chain).cloned().collect();
            crate::chainx::balance(&mine, *chain, &who).ok().map(|(w, _)| (*chain, w))
        })
        .collect()
}

pub fn swept_now(shell: &Shell) -> Vec<(u64, crate::rotatex::Settled)> {
    let Some((row, seat)) = shell.identities.as_ref().and_then(crate::identity::now_row) else { return Vec::new() };
    let Ok(s) = seat_in(row, seat) else { return Vec::new() };
    sweep_from(shell, &s).map(|(id, _, _)| crate::rotatex::settled_of(&shell.settings.swept, &id)).unwrap_or_default()
}

pub fn sweep_open(shell: &Shell) -> bool {
    let Some((row, seat)) = shell.identities.as_ref().and_then(crate::identity::now_row) else { return false };
    let Ok(s) = seat_in(row, seat) else { return false };
    let basis: Vec<u64> = shell.basis_chains().into_iter().map(|(c, _, _)| c).collect();
    sweep_from(shell, &s).is_some_and(|(id, old, _)| crate::rotatex::sweep(&basis, &left_for(shell, &old), &crate::rotatex::settled_of(&shell.settings.swept, &id), None).is_some())
}

fn priced(mine: &[crate::chainx::Endpoint], chain: u64, old: Address, to: Address) -> Result<crate::rotatex::Left, crate::fault::Fault> {
    use crate::rotatex::Amount;
    let (held, _) = crate::chainx::balance(mine, chain, &old)?;
    if held == 0 {
        return Ok(crate::rotatex::Left { chain, held, cost: 0 });
    }
    let ask = |value: u128| zikaron_send::one::estimate(mine, &old.0, &zikaron_send::one::Call::transfer(chain, to.0, value));
    let opening = crate::rotatex::sweep_opening(held);
    let (_, fees) = ask(opening)?;
    let draft = zikaron_anchor::send::Unsigned { chain_id: chain, nonce: u64::MAX, max_priority_fee: fees.priority, max_fee: fees.max_fee, gas: fees.gas_limit, to: to.0, value: opening, data: Vec::new() };
    let data = zikaron_send::one::data_fee(mine, &draft)?;
    match crate::rotatex::sweep_amount(held, fees.max_fee, data, (opening, fees.gas_limit), |value| ask(value).map(|(_, f)| f.gas_limit))? {
        Amount::Send { value, .. } => Ok(crate::rotatex::Left { chain, held, cost: held - value }),
        Amount::Short { cost } => Ok(crate::rotatex::Left { chain, held, cost }),
        Amount::Unsettled => Err(crate::fault::Fault::known(crate::fault::Known::GasRefused, format!("{chain} sweep"))),
    }
}

fn read_left(eps: &[crate::chainx::Endpoint], basis: &[(u64, Address)], old: Address, to: Address) -> Result<Vec<crate::rotatex::Left>, crate::fault::Fault> {
    basis
        .iter()
        .map(|(chain, _)| {
            let mine: Vec<crate::chainx::Endpoint> = eps.iter().filter(|e| e.chain == *chain).cloned().collect();
            priced(&mine, *chain, old, to)
        })
        .collect()
}

#[allow(clippy::type_complexity)]
fn read_after(eps: Vec<crate::chainx::Endpoint>, basis: Vec<(u64, Address)>, root: Address, before: Address, office: Address, old: Option<(String, Address, Address)>, kept: &[(u64, crate::rotatex::Settled)], out: Option<u64>) -> Result<(Vec<crate::rotatex::Chain>, Option<crate::rotatex::LeftRead>, Vec<(String, u64, crate::rotatex::Settled)>, Option<crate::fault::Fault>), crate::fault::Fault> {
    let chains = read_chains(eps.clone(), basis.clone(), root, before, office)?;
    let Some((id, old, to)) = old else { return Ok((chains, None, Vec::new(), None)) };
    match read_left(&eps, &basis, old, to) {
        Ok(left) => {
            let cs: Vec<u64> = basis.iter().map(|(c, _)| *c).filter(|c| Some(*c) != out).collect();
            let shorts = crate::rotatex::shorts(&cs, &left, kept).into_iter().map(|(c, s)| (id.clone(), c, s)).collect();
            Ok((chains, Some(crate::rotatex::LeftRead { old, left }), shorts, None))
        }
        Err(f) => Ok((chains, None, Vec::new(), Some(f))),
    }
}

fn out_now(eps: &[crate::chainx::Endpoint], chain: u64, tx: &str) -> Result<Option<u64>, crate::fault::Fault> {
    let mine: Vec<crate::chainx::Endpoint> = eps.iter().filter(|e| e.chain == chain).cloned().collect();
    let receipt = crate::chainx::ask(&mine, "eth_getTransactionReceipt", &zikaron::json::Value::Arr(vec![zikaron::json::Value::Str(tx.to_string())]))?;
    if let Some(status) = receipt.value.member("status").and_then(|s| s.as_str()).and_then(zikaron_send::node::wei) {
        return Ok(Some(u64::try_from(status).unwrap_or(0)));
    }
    let urls: Vec<crate::chainx::NodeAddr> = mine.iter().map(|e| e.url.clone()).collect();
    Ok(out_of(crate::chainx::tx_as_held(&urls, chain, tx)))
}

fn out_of(held: crate::chainx::AsHeld) -> Option<u64> {
    match held {
        crate::chainx::AsHeld::Nobody => Some(0),
        crate::chainx::AsHeld::Held(_) | crate::chainx::AsHeld::Unread | crate::chainx::AsHeld::NoNode => None,
    }
}

fn out_settled(eps: &[crate::chainx::Endpoint], out: &Option<(u64, u128, String)>) -> Result<(Vec<(u64, crate::rotatex::Settled)>, bool), crate::fault::Fault> {
    let Some((chain, value, tx)) = out else { return Ok((Vec::new(), false)) };
    Ok(match out_now(eps, *chain, tx)? {
        Some(1) => (vec![(*chain, crate::rotatex::Settled::Sent { value: *value, tx: tx.clone() })], false),
        Some(_) => (Vec::new(), true),
        None => (Vec::new(), false),
    })
}

fn moved_to(shell: &Shell, current: &Address) -> Option<(String, String)> {
    crate::rotatex::moved(current, last_move(shell, current), shell.rows_current(), shell.rot_written.clone())
}

fn queued_ids(shell: &Shell) -> Vec<String> {
    shell.queue.items.iter().map(|q| q.id.clone()).collect()
}

fn out_ids(shell: &Shell) -> Vec<String> {
    shell.queue.items.iter().filter(|q| q.step.in_flight()).map(|q| q.id.clone()).collect()
}

fn out_here(shell: &Shell, chain: u64) -> Result<(), crate::fault::Fault> {
    match &shell.rot_sweep_held {
        Some((c, _, tx)) if *c == chain => Err(crate::fault::Fault::known(crate::fault::Known::NotIncluded, tx.clone())),
        _ => Ok(()),
    }
}

pub fn start_stage(shell: &Shell) -> Option<crate::rotatex::Stage> {
    let (row, seat) = crate::identity::now_row(shell.identities.as_ref()?)?;
    seat_in(row, seat).ok().map(|s| may_start(shell, s.current))
}

fn may_start(shell: &Shell, current: Address) -> crate::rotatex::Stage {
    let basis: Vec<u64> = shell.basis_chains().into_iter().map(|(c, _, _)| c).collect();
    let chains = shell.rotation.clone().unwrap_or_default();
    let (on, queued, own) = start_owing(shell, current);
    let owing = crate::rotatex::Owing { on: &on, queued: &queued, own: own.as_deref() };
    crate::rotatex::stage(current, None, None, shell.audit_report_in_step(), &owing, &kept_local(shell), &basis, &chains, &faces_of(shell))
}

pub fn rotation_owed(shell: &Shell) -> Option<usize> {
    let (row, seat) = crate::identity::now_row(shell.identities.as_ref()?)?;
    let s = seat_in(row, seat).ok()?;
    let (on, queued, own) = start_owing(shell, s.current);
    let owing = crate::rotatex::Owing { on: &on, queued: &queued, own: own.as_deref() };
    Some(crate::rotatex::owed(shell.audit_report_in_step(), &owing, &kept_local(shell)))
}

fn start_owing(shell: &Shell, current: Address) -> (crate::chainx::OnChain, Vec<String>, Option<String>) {
    (shell.on_chain(), queued_ids(shell), moved_to(shell, &current).map(|(id, _)| id))
}

fn bookless(chains: &[(u64, crate::rotatex::Lack)]) -> crate::fault::Fault {
    let named: Vec<String> = chains.iter().map(|(c, why)| format!("{c}:{}", why.as_str())).collect();
    crate::fault::Fault::known(crate::fault::Known::RotateBookless, named.join(" "))
}

fn unseated_fault(n: Option<usize>) -> crate::fault::Fault {
    crate::fault::Fault::known(crate::fault::Known::RotateUnseated, n.map(|n| n.to_string()).unwrap_or_else(|| "unread".to_string()))
}

fn unentered_fault(chains: &[u64]) -> crate::fault::Fault {
    crate::fault::Fault::known(crate::fault::Known::RotateUnentered, chains.iter().map(u64::to_string).collect::<Vec<_>>().join(" "))
}

fn withdrawable(shell: &Shell, chain: u64) -> Result<(), crate::fault::Fault> {
    match shell.rotation.as_ref().and_then(|cs| cs.iter().find(|c| c.chain == chain)).map(|c| c.book) {
        Some(crate::rotatex::Book::Read { own, .. }) if crate::rotatex::declares(&own) => Ok(()),
        Some(crate::rotatex::Book::Read { .. }) => Err(crate::fault::Fault::known(crate::fault::Known::RotateOwed, format!("declared {chain}"))),
        _ => Err(crate::fault::Fault::known(crate::fault::Known::RotateOwed, format!("unread {chain}"))),
    }
}

fn owed_fault(n: usize) -> crate::fault::Fault {
    crate::fault::Fault::known(crate::fault::Known::RotateOwed, n.to_string())
}

fn read_chains(eps: Vec<crate::chainx::Endpoint>, basis: Vec<(u64, Address)>, root: Address, before: Address, office: Address) -> Result<Vec<crate::rotatex::Chain>, crate::fault::Fault> {
    use crate::rotatex::{Book, Chain};
    crate::chainx::speak();
    let mut out = Vec::new();
    for (chain, _) in basis {
        let mine: Vec<crate::chainx::Endpoint> = eps.iter().filter(|e| e.chain == chain).cloned().collect();
        let book = match zikaron_send::book::state(&mine, chain)? {
            zikaron_send::book::State::Off => Book::Off,
            zikaron_send::book::State::Wrong => Book::Wrong,
            zikaron_send::book::State::At(at) => {
                let pending = zikaron_send::book::address(&mine, chain, &at, zikaron_anchor::book::Call::Pending, &before.0)?;
                let own = if office == before { pending } else { zikaron_send::book::address(&mine, chain, &at, zikaron_anchor::book::Call::Pending, &office.0)? };
                let held = zikaron_send::book::held(&mine, chain, &at, &root.0)?;
                Book::Read { pending, current: held.current, root: held.root, own }
            }
        };
        out.push(Chain { chain, book });
    }
    Ok(out)
}

fn after_send(read: Result<Vec<crate::rotatex::Chain>, crate::fault::Fault>, prior: Vec<crate::rotatex::Chain>, went: bool, chain: u64, says: crate::rotatex::Fell) -> (Vec<crate::rotatex::Chain>, Option<crate::fault::Fault>) {
    let (chains, unread) = match read {
        Ok(c) => (c, None),
        Err(f) => (prior, Some(f)),
    };
    (if went { crate::rotatex::after_fell(chains, chain, says) } else { chains }, unread)
}

fn reverted_of(chain: u64, step: &'static str, f: &zikaron_send::one::Fell, limit: u64) -> Option<crate::rotatex::Reverted> {
    (f.status != 1).then(|| crate::rotatex::Reverted { chain, step, tx: f.tx.clone(), used: f.gas_used, limit })
}

pub(super) fn failed_in_block(f: &zikaron_send::one::Fell, limit: u64) -> crate::fault::Fault {
    crate::fault::Fault::known(crate::fault::Known::SendFailed, format!("{} gasUsed={} gasLimit={limit}", f.tx, f.gas_used.map(|g| g.to_string()).unwrap_or_default()))
}

pub(super) fn fell_said(f: &zikaron_send::one::Fell) -> String {
    match f.gas_used {
        Some(g) => format!("{} · gasUsed={g} · status={}", f.tx, f.status),
        None => format!("{} · status={}", f.tx, f.status),
    }
}

pub(super) fn spawn_rotate(shell: &mut Shell, work: impl FnOnce() -> Result<Done, crate::fault::Fault> + Send + 'static) -> Spawned {
    let spawned = shell.tasks.spawn(Kind::Rotate, work);
    if spawned == Spawned::Started {
        shell.rot_home = shell.home.as_ref().map(|h| h.root().to_path_buf());
    }
    spawned
}

pub(super) fn rotate(shell: &mut Shell, step: RotateStep) -> Result<Applied, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    use crate::rotatex::{Source, Stage};
    match step {
        RotateStep::Sweep { .. } | RotateStep::Leave { .. } => shell.may_move_funds()?,
        _ if step.uses_key() => shell.may_write_entries()?,
        _ => {}
    }
    let s = seat_now()?;
    let book_root = ledger_root(shell).unwrap_or(s.root);
    let basis: Vec<(u64, Address)> = shell.basis_chains().into_iter().map(|(c, r, _)| (c, r)).collect();
    let eps = shell.endpoints.clone();
    let before = crate::rotatex::holder(s.current, s.next, shell.rot_given, moved_to(shell, &s.current).as_ref());
    let spawn = |shell: &mut Shell, work: Box<dyn FnOnce() -> Result<Done, Fault> + Send>| -> Applied {
        match spawn_rotate(shell, work) {
            Spawned::Started => Applied::Started(Kind::Rotate),
            Spawned::InFlight => Applied::Refused(Kind::Rotate),
        }
    };
    match step {
        RotateStep::Look => {
            let root = book_root;
            let old = sweep_from(shell, &s);
            let (fund, eps_fund, basis_fund) = (s.current, eps.clone(), basis.clone());
            let kept = old.as_ref().map(|(id, _, _)| crate::rotatex::settled_of(&shell.settings.swept, id)).unwrap_or_default();
            let out = shell.rot_sweep_held.clone().filter(|_| old.is_some());
            let started = spawn(
                shell,
                Box::new(move || {
                    let (fell, gone) = out_settled(&eps, &out)?;
                    let mut kept = kept;
                    kept.extend(fell.iter().cloned());
                    let id = old.as_ref().map(|(id, _, _)| id.clone()).unwrap_or_default();
                    let still = out.as_ref().filter(|_| fell.is_empty() && !gone).map(|(c, _, _)| *c);
                    read_after(eps, basis, root, before, fund, old, &kept, still).map(|(chains, left, settled, unread)| {
                        let settled = fell.into_iter().map(|(c, s)| (id.clone(), c, s)).chain(settled).collect();
                        let fresh = fresh_of(&eps_fund, &basis_fund, fund);
                        Done::Rotation { chains, said: None, unread, left, settled, gone, reverted: None, fresh, withdrawn: false }
                    })
                }),
            );
            shell.rot_looking |= started == Applied::Started(Kind::Rotate);
            Ok(started)
        }
        RotateStep::Vet { to } => {
            let who = Address::parse(&to).ok_or_else(|| Fault::known(Known::AddressShape, to.trim().to_string()))?;
            shell.rot_vetted = None;
            let items = shell.home.as_ref().map(|h| h.ledger()?.pile().map(|p| p.items)).transpose()?.unwrap_or_default();
            let reg = crate::register::view()?;
            let ground = ground_read(shell)?;
            let lanes = shell.basis_chains();
            Ok(spawn(
                shell,
                Box::new(move || {
                    crate::chainx::speak();
                    fit_here(&items, &reg, &who)?;
                    for (chain, registry, from) in lanes {
                        let mine: Vec<crate::chainx::Endpoint> = eps.iter().filter(|e| e.chain == chain).cloned().collect();
                        let (head, _) = crate::chainx::head_block(&mine, chain)?;
                        let g = crate::auditx::Ground { chain, registry, from_block: from, to_block: head.max(from), earlier: Vec::new(), ..ground.clone() };
                        let seen = crate::succeedx::look_at(&mine, &g, &who)?;
                        if seen.anchors > 0 {
                            return Err(crate::succeedx::taken(Fault::known(Known::AlreadyRooted, who.hex()), crate::lang::Key::FaultWhatGivenHasLedger));
                        }
                        crate::succeedx::seated_in_book(&mine, chain, &who)?;
                    }
                    Ok(Done::RotateVetted { to: who })
                }),
            ))
        }
        RotateStep::Hand { to } => {
            no_key_to_change(&s)?;
            let who = Address::parse(&to).ok_or_else(|| Fault::known(Known::AddressShape, to.trim().to_string()))?;
            if shell.rot_vetted != Some(who) {
                return Err(Fault::known(Known::NotAudited, who.hex()));
            }
            if let Some(at @ Stage::Sweep { .. }) = rotation_stage(shell) {
                return Err(Fault::known(Known::RotateOwed, at.as_str().to_string()));
            }
            match may_start(shell, s.current) {
                Stage::Ready if s.next.is_none() => {}
                Stage::Bookless { chains } => return Err(bookless(&chains)),
                Stage::Owed { n } => return Err(owed_fault(n)),
                Stage::Unseated { n } => return Err(unseated_fault(n)),
                Stage::Unentered { chains } => return Err(unentered_fault(&chains)),
                other => return Err(Fault::known(Known::RotateOwed, other.as_str().to_string())),
            }
            shell.rot_vetted = None;
            shell.rot_given = Some(who);
            Ok(Applied::Rotated { stage: rotation_stage(shell).map(|x| x.as_str()).unwrap_or_default().to_string() })
        }
        RotateStep::Begin { source, answers } => {
            no_key_to_change(&s)?;
            if s.next.is_some() {
                return Ok(Applied::Rotated { stage: rotation_stage(shell).map(|x| x.as_str()).unwrap_or_default().to_string() });
            }
            if let Some(at @ Stage::Sweep { .. }) = rotation_stage(shell) {
                return Err(Fault::known(Known::RotateOwed, at.as_str().to_string()));
            }
            match may_start(shell, s.current) {
                Stage::Ready => {}
                Stage::Behind => {
                    if !super::audit::recount(shell) {
                        let _ = start_audit(shell);
                    }
                    return Err(Fault::known(Known::AuditBehind, String::new()));
                }
                Stage::Owed { n } => return Err(Fault::known(Known::RotateOwed, n.to_string())),
                Stage::Bookless { chains } => return Err(bookless(&chains)),
                Stage::Unseated { n } => return Err(unseated_fault(n)),
                Stage::Unentered { chains } => return Err(unentered_fault(&chains)),
                other => {
                    let (root, before) = (book_root, s.current);
                    let _ = spawn(shell, Box::new(move || read_chains(eps, basis, root, before, before).map(|chains| Done::Rotation { chains, said: None, unread: None, left: None, settled: Vec::new(), gone: false, reverted: None, fresh: Vec::new(), withdrawn: false })));
                    return Ok(Applied::Rotated { stage: other.as_str().to_string() });
                }
            }
            let malformed = || Fault::known(Known::KeyMalformed, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail013).to_string()));
            let group = |g: u64| u32::try_from(g).ok().filter(|g| *g < crate::family::HARDENED).ok_or_else(malformed);
            let key = match (source, s.row.kind()) {
                (Source::SameWords, crate::identity::Kind::Words) => {
                    let seed = crate::keybox::get(&crate::places::seed_slot(&s.row.id))?.ok_or_else(|| Fault::known(Known::KeychainMissing, crate::places::seed_slot(&s.row.id)))?;
                    let mut e = <[u8; crate::family::ENTROPY_BYTES]>::try_from(seed.as_slice()).map_err(|_| malformed())?;
                    let k = crate::key::derived_at(&e, s.seat, group(s.generation + 1)?);
                    zikaron_os::wipe(&mut e);
                    k.ok_or_else(malformed)?
                }
                (Source::NewWords, crate::identity::Kind::Words) => {
                    let pending = crate::keybox::get(&crate::places::seed_next_slot(&s.row.id))?;
                    match pending {
                        Some(e) => {
                            let mut e = <[u8; crate::family::ENTROPY_BYTES]>::try_from(e.as_slice()).map_err(|_| malformed())?;
                            let k = crate::key::derived_at(&e, s.seat, crate::family::GROUP);
                            zikaron_os::wipe(&mut e);
                            k.ok_or_else(malformed)?
                        }
                        None => {
                            new_words_may_start(&s.row)?;
                            let f = shell.new_words.as_ref().ok_or_else(|| Fault::known(Known::PhraseConfirm, String::new()))?;
                            crate::identity::confirm(f, &answers)?;
                            let k = crate::key::derived_at(f.entropy(), s.seat, crate::family::GROUP).ok_or_else(malformed)?;
                            crate::keybox::put(&crate::places::seed_next_slot(&s.row.id), f.entropy())?;
                            k
                        }
                    }
                }
                (Source::Drawn, crate::identity::Kind::Existing) => crate::key::generate()?,
                _ => return Err(Fault::known(Known::IdentitiesShape, source.as_str().to_string())),
            };
            let next = key.address().ok_or_else(malformed)?;
            crate::key::install_at(&crate::places::key_slot(&next), &key)?;
            let id = s.row.id.clone();
            let seat = s.seat;
            let reg = crate::register::change(|reg| {
                crate::identity::set_next(reg, &id, seat, Some(next))?;
                Ok(reg.clone())
            })?;
            shell.seat_identities(Some(reg));
            shell.new_words = None;
            shell.rotation = None;
            Ok(Applied::Rotated { stage: rotation_stage(shell).map(|x| x.as_str()).unwrap_or_default().to_string() })
        }
        RotateStep::Declare { chain } | RotateStep::Accede { chain } | RotateStep::Withdraw { chain } => {
            let step_name = match step {
                RotateStep::Declare { .. } => "declare",
                RotateStep::Withdraw { .. } => "withdraw",
                _ => "accede",
            };
            if matches!(step, RotateStep::Withdraw { .. }) {
                withdrawable(shell, chain)?;
            }
            let secret = signing_key(shell, crate::sign::Use::Anchor)?;
            let ask = crate::exitgate::ask_of(shell)?;
            let urls: Vec<crate::chainx::NodeAddr> = eps.iter().filter(|e| e.chain == chain).map(|e| e.url.clone()).collect();
            let book = zikaron_anchor::book::at(chain);
            let (call, says) = match (&step, s.next.or(shell.rot_given), book) {
                (RotateStep::Declare { .. }, Some(next), Some(at)) => (zikaron_send::one::Call::book(chain, at, zikaron_anchor::book::Call::Declare, &next.0), crate::rotatex::Fell::Declared { next }),
                (RotateStep::Accede { .. }, None, Some(at)) => (zikaron_send::one::Call::book(chain, at, zikaron_anchor::book::Call::Accede, &before.0), crate::rotatex::Fell::Acceded { current: s.current }),
                (RotateStep::Withdraw { .. }, _, Some(at)) => (zikaron_send::one::Call::book(chain, at, zikaron_anchor::book::Call::Declare, &crate::rotatex::WITHDRAWN), crate::rotatex::Fell::Withdrawn),
                _ => return Err(Fault::known(Known::BookCode, chain.to_string())),
            };
            let root = book_root;
            let prior = shell.rotation.clone().unwrap_or_default();
            let old = sweep_from(shell, &s);
            let (fund, eps_fund, basis_fund) = (s.current, eps.clone(), basis.clone());
            let kept = old.as_ref().map(|(id, _, _)| crate::rotatex::settled_of(&shell.settings.swept, id)).unwrap_or_default();
            let wait = std::time::Duration::from_secs(60);
            let started = spawn(
                shell,
                Box::new(move || {
                    crate::chainx::speak();
                    let mine: Vec<crate::chainx::Endpoint> = eps.iter().filter(|e| e.chain == chain).cloned().collect();
                    if !call.data.is_empty() && zikaron_send::book::state(&mine, chain)? != zikaron_send::book::State::At(call.to) {
                        return Err(Fault::known(Known::BookCode, chain.to_string()));
                    }
                    let from = zikaron_send::one::PlainSigner::from(&secret).ok_or_else(|| Fault::known(Known::KeyMalformed, String::new()))?;
                    let (_, fees) = zikaron_send::one::estimate(&mine, &from, &call)?;
                    let limit = fees.gas_limit;
                    let pass = crate::exitgate::pass(&ask)?;
                    let fell = zikaron_send::one::send(zikaron_send::one::Through::Gate(&pass), &urls, &call, &secret, fees, wait)?;
                    let read = read_after(eps, basis, root, before, fund, old, &kept, None);
                    let (left, settled, left_unread) = match &read {
                        Ok((_, l, s, u)) => (l.clone(), s.clone(), u.clone()),
                        Err(_) => (None, Vec::new(), None),
                    };
                    let (chains, unread) = after_send(read.map(|(c, _, _, _)| c), prior, fell.status == 1, chain, says);
                    Ok(Done::Rotation { chains, said: Some(fell_said(&fell)), unread: unread.or(left_unread), left, settled, gone: false, reverted: reverted_of(chain, step_name, &fell, limit), fresh: fresh_of(&eps_fund, &basis_fund, fund), withdrawn: step_name == "withdraw" && fell.status == 1 })
                }),
            );
            if started == Applied::Started(Kind::Rotate) {
                shell.rot_sending = Some(chain);
            }
            Ok(started)
        }
        RotateStep::Sweep { chain, value } => {
            let Some(Stage::Sweep { chain: at, .. }) = rotation_stage(shell) else {
                return Err(Fault::known(Known::RotateOwed, rotation_stage(shell).map(|x| x.as_str()).unwrap_or_default().to_string()));
            };
            if at != chain {
                return Err(Fault::known(Known::RotateOwed, format!("sweep {at}")));
            }
            out_here(shell, chain)?;
            let (id, old, to) = sweep_from(shell, &s).ok_or_else(|| Fault::known(Known::RotateOwed, String::new()))?;
            let slot = crate::places::key_slot(&old);
            let secret = crate::key::load(Some(&slot))?.ok_or_else(|| Fault::known(Known::KeychainMissing, slot.clone()))?;
            let ask = crate::exitgate::ask_of(shell)?;
            let urls: Vec<crate::chainx::NodeAddr> = eps.iter().filter(|e| e.chain == chain).map(|e| e.url.clone()).collect();
            let root = book_root;
            let (fund, eps_fund, basis_fund) = (s.current, eps.clone(), basis.clone());
            let kept = crate::rotatex::settled_of(&shell.settings.swept, &id);
            let prior = shell.rotation.clone().unwrap_or_default();
            let wait = std::time::Duration::from_secs(60);
            let started = spawn(
                shell,
                Box::new(move || {
                    crate::chainx::speak();
                    let mine: Vec<crate::chainx::Endpoint> = eps.iter().filter(|e| e.chain == chain).cloned().collect();
                    let (held, _) = crate::chainx::balance(&mine, chain, &old)?;
                    let call = zikaron_send::one::Call::transfer(chain, to.0, value);
                    let its = |t: zikaron_send::Trouble| -> Fault {
                        let f = Fault::from(t);
                        match f.which() {
                            Some(Known::InsufficientFunds) => Fault::known(Known::InsufficientFunds, format!("{} {}", f.tail(), crate::action::funds_key_word(&old))),
                            _ => f,
                        }
                    };
                    let fitted = if value == 0 || value > held {
                        Err(value)
                    } else {
                        let (_, fees) = zikaron_send::one::estimate(&mine, &old.0, &call).map_err(its)?;
                        let draft = zikaron_anchor::send::Unsigned { chain_id: chain, nonce: u64::MAX, max_priority_fee: fees.priority, max_fee: fees.max_fee, gas: fees.gas_limit, to: to.0, value, data: Vec::new() };
                        let data = zikaron_send::one::data_fee(&mine, &draft).map_err(its)?;
                        crate::rotatex::fit(held, value, data, fees).ok_or(value.saturating_add(fees.cap_wei()).saturating_add(data))
                    };
                    let fees = match fitted {
                        Ok(fees) => fees,
                        Err(need) => {
                            let unread = Fault::known(Known::InsufficientFunds, crate::action::funds_tail_of(need, held, &old));
                            let (chains, left, settled, _) = read_after(eps, basis, root, before, fund, Some((id, old, to)), &kept, None)?;
                            return Ok(Done::Rotation { chains, said: None, unread: Some(unread), left, settled, gone: false, reverted: None, fresh: fresh_of(&eps_fund, &basis_fund, fund), withdrawn: false });
                        }
                    };
                    let pass = crate::exitgate::pass(&ask)?;
                    let fell = zikaron_send::one::send(zikaron_send::one::Through::Gate(&pass), &urls, &call, &secret, fees, wait).map_err(its)?;
                    let reverted = reverted_of(chain, "sweep", &fell, fees.gas_limit);
                    let mut settled = Vec::new();
                    if fell.status == 1 {
                        settled.push((id.clone(), chain, crate::rotatex::Settled::Sent { value, tx: fell.tx.clone() }));
                    }
                    let mut kept = kept;
                    kept.extend(settled.iter().map(|(_, c, s)| (*c, s.clone())));
                    match read_after(eps, basis, root, before, fund, Some((id, old, to)), &kept, None) {
                        Ok((chains, left, more, unread)) => {
                            settled.extend(more);
                            Ok(Done::Rotation { chains, said: Some(fell_said(&fell)), unread, left, settled, gone: false, reverted, fresh: fresh_of(&eps_fund, &basis_fund, fund), withdrawn: false })
                        }
                        Err(f) => Ok(Done::Rotation { chains: prior, said: Some(fell_said(&fell)), unread: Some(f), left: None, settled, gone: false, reverted, fresh: Vec::new(), withdrawn: false }),
                    }
                }),
            );
            if started == Applied::Started(Kind::Rotate) {
                shell.rot_sending = Some(chain);
                shell.rot_sweep_out = Some((chain, value));
            }
            Ok(started)
        }
        RotateStep::Leave { chain } => {
            let Some(Stage::Sweep { chain: at, .. }) = rotation_stage(shell) else {
                return Err(Fault::known(Known::RotateOwed, rotation_stage(shell).map(|x| x.as_str()).unwrap_or_default().to_string()));
            };
            if at != chain {
                return Err(Fault::known(Known::RotateOwed, format!("sweep {at}")));
            }
            out_here(shell, chain)?;
            let (id, old, _) = sweep_from(shell, &s).ok_or_else(|| Fault::known(Known::RotateOwed, String::new()))?;
            let held = left_for(shell, &old).iter().find(|x| x.chain == chain).map(|x| x.held);
            shell.keep_swept(&[(id, chain, crate::rotatex::Settled::Left { held })]);
            Ok(Applied::Rotated { stage: rotation_stage(shell).map(|x| x.as_str()).unwrap_or_default().to_string() })
        }
        RotateStep::SweepTo { to } => {
            if shell.handed.is_none() {
                return Err(Fault::known(Known::RotateOwed, rotation_stage(shell).map(|x| x.as_str()).unwrap_or_default().to_string()));
            }
            let who = Address::parse(&to).ok_or_else(|| Fault::known(Known::AddressShape, to.trim().to_string()))?;
            if let Some((chain, _, _)) = &shell.rot_sweep_held {
                out_here(shell, *chain)?;
            }
            let now = shell.rot_sweep_to.or_else(|| shell.handed.as_deref().and_then(Address::parse));
            if now != Some(who) {
                shell.rot_sweep_to = Some(who);
                shell.rot_left = None;
                shell.rot_left_of = None;
            }
            Ok(Applied::Rotated { stage: rotation_stage(shell).map(|x| x.as_str()).unwrap_or_default().to_string() })
        }
        RotateStep::Write => {
            let given = shell.rot_given;
            let next = s.next.or(given).ok_or_else(|| Fault::known(Known::NoIdentity, format!("{} next", s.row.id)))?;
            match rotation_stage(shell) {
                Some(Stage::Write) => {}
                Some(Stage::Behind) => {
                    if !super::audit::recount(shell) {
                        let _ = start_audit(shell);
                    }
                    return Err(Fault::known(Known::AuditBehind, String::new()));
                }
                Some(Stage::Bookless { chains }) => return Err(bookless(&chains)),
                Some(Stage::Unseated { n }) => return Err(unseated_fault(n)),
                Some(Stage::Unentered { chains }) => return Err(unentered_fault(&chains)),
                other => return Err(Fault::known(Known::RotateOwed, other.map(|x| x.as_str()).unwrap_or_default().to_string())),
            }
            let kind = if given.is_some() { crate::succeedx::KIND_HANDOVER } else { crate::succeedx::KIND_ROTATION };
            if given.is_some() {
                let items = shell.home.as_ref().map(|h| h.ledger()?.pile().map(|p| p.items)).transpose()?.unwrap_or_default();
                fit_here(&items, &crate::register::view()?, &next)?;
            }
            let (head, secret) = ready_to_append(shell)?;
            let (effective, statement) = crate::succeedx::fill(kind, "", "", (shell.clock)());
            let body = crate::succeedx::succession_body(&next.hex(), kind, &effective, &statement)?;
            let sealed = crate::entryx::seal(&secret, zikaron_glue::book::Kind::Succession.as_str(), head.0 + 1, Some(&head.1), body)?;
            drop(secret);
            let id = land_sealed(shell, sealed)?;
            shell.rot_written = Some((id.clone(), s.current.hex()));
            if given.is_some() {
                shell.rot_given = None;
                shell.rotation = None;
                let n = queue_it(shell, &id);
                shell.handed = Some(next.hex());
                shell.reread(crate::shell::Reread::Wrote);
                return Ok(Applied::Queued { id, queued: n.queued, next: n.next });
            }
            let (rid, seat) = (s.row.id.clone(), s.seat);
            super::office::settle(shell, || {
                let reg = crate::register::change(|reg| {
                    crate::identity::take_office(reg, &rid, seat)?;
                    Ok(reg.clone())
                })?;
                Ok(words_complete(&reg, &rid)?.unwrap_or(reg))
            })?;
            shell.rotation = None;
            let n = queue_it(shell, &id);
            shell.reread(crate::shell::Reread::Wrote);
            Ok(Applied::Queued { id, queued: n.queued, next: n.next })
        }
        RotateStep::Enroll { chain } => {
            let current = signing_key(shell, crate::sign::Use::Anchor)?;
            let root_key = crate::key::load(Some(&crate::places::key_slot(&book_root)))?.ok_or_else(|| Fault::known(Known::RootKeyAbsent, book_root.hex()))?;
            let at = zikaron_anchor::book::at(chain).ok_or_else(|| Fault::known(Known::BookCode, chain.to_string()))?;
            let ask = crate::exitgate::ask_of(shell)?;
            let urls: Vec<crate::chainx::NodeAddr> = eps.iter().filter(|e| e.chain == chain).map(|e| e.url.clone()).collect();
            let (root, office) = (book_root, s.current);
            let wait = std::time::Duration::from_secs(60);
            Ok(spawn(
                shell,
                Box::new(move || {
                    use zikaron_anchor::book::Call as B;
                    crate::chainx::speak();
                    let mine: Vec<crate::chainx::Endpoint> = eps.iter().filter(|e| e.chain == chain).cloned().collect();
                    if zikaron_send::book::state(&mine, chain)? != zikaron_send::book::State::At(at) {
                        return Err(Fault::known(Known::BookCode, chain.to_string()));
                    }
                    let pass = crate::exitgate::pass(&ask)?;
                    let mut said: Vec<String> = Vec::new();
                    let plan = enrolment_read(&mine, chain, &at, &root, &office)?.1.map_err(|c| closed_fault(chain, &root, &office, c))?;
                    if plan == crate::rotatex::Enrol::Declare {
                        let declare = zikaron_send::one::Call::book(chain, at, B::Declare, &office.0);
                        let (_, fees) = zikaron_send::one::estimate(&mine, &root.0, &declare)?;
                        let (have, _) = crate::chainx::balance(&mine, chain, &root)?;
                        let need = u128::from(fees.gas_limit).saturating_mul(u128::from(fees.max_fee)).saturating_mul(2);
                        if have < need {
                            let lack = need.saturating_add(need / 4).saturating_sub(have);
                            let fund = zikaron_send::one::Call::transfer(chain, root.0, lack);
                            let (_, ff) = zikaron_send::one::estimate(&mine, &office.0, &fund)?;
                            let fell = zikaron_send::one::send(zikaron_send::one::Through::Gate(&pass), &urls, &fund, &current, ff, wait)?;
                            if fell.status != 1 {
                                return Err(failed_in_block(&fell, ff.gas_limit));
                            }
                            said.push(fell_said(&fell));
                        }
                        let fell = zikaron_send::one::send(zikaron_send::one::Through::Gate(&pass), &urls, &declare, &root_key, fees, wait)?;
                        if fell.status != 1 {
                            return Err(failed_in_block(&fell, fees.gas_limit));
                        }
                        said.push(fell_said(&fell));
                    }
                    if plan != crate::rotatex::Enrol::Held {
                        let accede = zikaron_send::one::Call::book(chain, at, B::Accede, &root.0);
                        let (_, fees) = zikaron_send::one::estimate(&mine, &office.0, &accede)?;
                        let fell = zikaron_send::one::send(zikaron_send::one::Through::Gate(&pass), &urls, &accede, &current, fees, wait)?;
                        if fell.status != 1 {
                            return Err(failed_in_block(&fell, fees.gas_limit));
                        }
                        said.push(fell_said(&fell));
                    }
                    let basis_here: Vec<(u64, Address)> = basis.iter().filter(|(c, _)| *c == chain).copied().collect();
                    read_chains(eps, basis_here, root, office, office).map(|chains| Done::Rotation { chains, said: Some(said.join(" ")), unread: None, left: None, settled: Vec::new(), gone: false, reverted: None, fresh: Vec::new(), withdrawn: false })
                }),
            ))
        }
        RotateStep::Cancel => {
            if let Some(given) = shell.rot_given {
                let told = shell.rotation.as_ref().and_then(|cs| cs.iter().find_map(|c| match c.book {
                    crate::rotatex::Book::Read { own, .. } if own == given.0 => Some(c.chain),
                    _ => None,
                }));
                if let Some(chain) = told {
                    let started = rotate(shell, RotateStep::Withdraw { chain })?;
                    shell.rot_given = None;
                    return Ok(started);
                }
                shell.rot_given = None;
                return Ok(Applied::Rotated { stage: rotation_stage(shell).map(|x| x.as_str()).unwrap_or_default().to_string() });
            }
            let Some(next) = s.next else { return Ok(Applied::Rotated { stage: String::new() }) };
            let (rid, seat) = (s.row.id.clone(), s.seat);
            let reg = crate::register::change(|reg| {
                crate::identity::set_next(reg, &rid, seat, None)?;
                Ok(reg.clone())
            })?;
            crate::keybox::drop_item(&crate::places::key_slot(&next))?;
            if !words_held(&reg, &rid)? {
                crate::keybox::drop_item(&crate::places::seed_next_slot(&rid))?;
                crate::keybox::drop_item(&crate::places::seed_reseal_slot(&rid))?;
            }
            shell.seat_identities(Some(reg));
            shell.rotation = None;
            Ok(Applied::Rotated { stage: rotation_stage(shell).map(|x| x.as_str()).unwrap_or_default().to_string() })
        }
    }
}

fn fit_here(items: &[Vec<u8>], reg: &crate::identity::Registry, to: &Address) -> Result<(), crate::fault::Fault> {
    if let Ok(read) = zikaron_glue::book::read(items, &zikaron_glue::book::empty_fragment()) {
        if !read.may_succeed(&to.hex()) {
            return Err(crate::succeedx::taken(crate::fault::Fault::known(crate::fault::Known::AlreadyRooted, to.hex()), crate::lang::Key::FaultWhatGivenInLineage));
        }
    }
    crate::succeedx::seated_here(reg, to)
}

fn no_key_to_change(s: &Seat) -> Result<(), crate::fault::Fault> {
    use crate::identity::SeatLedger;
    match crate::identity::seat_ledger(&s.row, s.seat) {
        SeatLedger::Holds => Ok(()),
        SeatLedger::Unchecked => Err(crate::fault::Fault::known(crate::fault::Known::LedgerNotFetched, s.root.hex())),
        SeatLedger::None => Err(crate::fault::Fault::known(crate::fault::Known::SeatUnhomed, s.root.hex())),
    }
}

pub fn old_words_held_elsewhere(reg: &crate::identity::Registry, id: &str, seat: crate::roles::Role) -> Option<bool> {
    let next = crate::keybox::get(&crate::places::seed_next_slot(id)).ok().flatten()?;
    let mut e = <[u8; crate::family::ENTROPY_BYTES]>::try_from(next.as_slice()).ok()?;
    let row = reg.rows.iter().find(|r| r.id.eq_ignore_ascii_case(id));
    let held = row.is_some_and(|r| {
        r.seats().into_iter().filter(|x| *x != seat && crate::identity::seat_ledger(r, *x) != crate::identity::SeatLedger::None).any(|x| crate::key::derived_at(&e, x, crate::family::GROUP).and_then(|k| k.address()) != r.current(x))
    });
    zikaron_os::wipe(&mut e);
    Some(held)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordsNote {
    Kept,
    OthersFollow,
}

pub fn words_note(reg: &crate::identity::Registry, id: &str, seat: crate::roles::Role) -> Option<WordsNote> {
    if old_words_held_elsewhere(reg, id, seat)? {
        return Some(WordsNote::Kept);
    }
    let row = reg.rows.iter().find(|r| r.id.eq_ignore_ascii_case(id))?;
    row.seats().into_iter().any(|x| x != seat && crate::identity::seat_ledger(row, x) == crate::identity::SeatLedger::None).then_some(WordsNote::OthersFollow)
}

fn new_words_may_start(row: &crate::identity::Row) -> Result<(), crate::fault::Fault> {
    use crate::identity::SeatLedger;
    use crate::roles::Role;
    for seat in row.seats() {
        let root = row.address(seat).map(|a| a.hex()).unwrap_or_default();
        match crate::identity::seat_ledger(row, seat) {
            SeatLedger::Holds => continue,
            SeatLedger::Unchecked => return Err(crate::fault::Fault::known(crate::fault::Known::LedgerNotFetched, root)),
            SeatLedger::None => {}
        }
        if seat == Role::Author || grants_name(&root) {
            return Err(crate::fault::Fault::known(crate::fault::Known::SeatUnhomed, root));
        }
    }
    Ok(())
}

fn grants_name(root: &str) -> bool {
    let Ok(reg) = crate::register::view() else { return false };
    reg.rows.iter().flat_map(|r| r.seats().into_iter().filter_map(move |s| r.home(s))).any(|h| {
        crate::home::Home::open(&h).ok().and_then(|home| crate::grantx::table(&home).ok()).is_some_and(|rows| rows.iter().any(|g| g.grantee.eq_ignore_ascii_case(root)))
    })
}

fn words_complete(reg: &crate::identity::Registry, id: &str) -> Result<Option<crate::identity::Registry>, crate::fault::Fault> {
    let Some(next) = crate::keybox::get(&crate::places::seed_next_slot(id))? else { return Ok(None) };
    let mut e = <[u8; crate::family::ENTROPY_BYTES]>::try_from(next.as_slice()).map_err(|_| crate::fault::Fault::known(crate::fault::Known::KeyMalformed, String::new()))?;
    let row = reg.rows.iter().find(|r| r.id.eq_ignore_ascii_case(id)).cloned().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoIdentity, id.to_string()))?;
    let key_of = |seat| crate::key::derived_at(&e, seat, crate::family::GROUP);
    let done = row.seats().into_iter().all(|s| match crate::identity::seat_ledger(&row, s) {
        crate::identity::SeatLedger::Holds => key_of(s).and_then(|k| k.address()) == row.current(s),
        crate::identity::SeatLedger::Unchecked => false,
        crate::identity::SeatLedger::None => true,
    });
    if !done {
        zikaron_os::wipe(&mut e);
        return Ok(None);
    }
    let mut moved: Option<crate::key::Secret> = None;
    for seat in row.seats().into_iter().filter(|s| crate::identity::seat_ledger(&row, *s) == crate::identity::SeatLedger::None) {
        moved = key_of(seat).filter(|k| k.address() != row.current(seat));
    }
    let reg = match moved {
        Some(k) => {
            let new = k.address().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::KeyMalformed, String::new()))?;
            crate::key::install_at(&crate::places::key_slot(&new), &k)?;
            let rid = id.to_string();
            match crate::register::change(move |reg| {
                crate::identity::reroot(reg, &rid, new)?;
                Ok(reg.clone())
            }) {
                Ok(r) => r,
                Err(f) => {
                    zikaron_os::wipe(&mut e);
                    return Err(f);
                }
            }
        }
        None => reg.clone(),
    };
    let reseal = crate::places::seed_reseal_slot(id);
    let owed = crate::keybox::present(&reseal)? || crate::keybox::primary()?.is_some_and(|(p, _)| p.eq_ignore_ascii_case(id));
    let settled = (|| {
        if owed {
            crate::keybox::put(&reseal, &[1])?;
        }
        crate::keybox::drop_item(&crate::places::seed_slot(id))?;
        crate::keybox::put(&crate::places::seed_slot(id), &e)?;
        if owed {
            if crate::keybox::primary()?.is_some_and(|(p, _)| p.eq_ignore_ascii_case(id)) {
                crate::keybox::drop_recovery(id)?;
            }
            crate::keybox::add_recovery(id, crate::keybox::PrimaryKind::Words, &e)?;
            crate::keybox::drop_item(&reseal)?;
        }
        Ok::<(), crate::fault::Fault>(())
    })();
    zikaron_os::wipe(&mut e);
    settled?;
    crate::keybox::drop_item(&crate::places::seed_next_slot(id))?;
    Ok(Some(reg))
}

pub fn words_settle(reg: &crate::identity::Registry) -> Result<Option<crate::identity::Registry>, crate::fault::Fault> {
    let mut out: Option<crate::identity::Registry> = None;
    let ids: Vec<String> = reg.rows.iter().map(|r| r.id.clone()).collect();
    for id in ids {
        if words_pending(&id) {
            if let Some(r) = words_complete(out.as_ref().unwrap_or(reg), &id)? {
                out = Some(r);
            }
        }
    }
    Ok(out)
}

fn words_held(reg: &crate::identity::Registry, id: &str) -> Result<bool, crate::fault::Fault> {
    let Some(next) = crate::keybox::get(&crate::places::seed_next_slot(id))? else { return Ok(false) };
    let Ok(mut e) = <[u8; crate::family::ENTROPY_BYTES]>::try_from(next.as_slice()) else { return Ok(false) };
    let held = reg.rows.iter().find(|r| r.id.eq_ignore_ascii_case(id)).is_some_and(|r| r.seats().into_iter().any(|s| crate::key::derived_at(&e, s, crate::family::GROUP).and_then(|k| k.address()) == r.current(s)));
    zikaron_os::wipe(&mut e);
    Ok(held)
}

pub fn words_pending(id: &str) -> bool {
    crate::keybox::present(&crate::places::seed_next_slot(id)).unwrap_or(false)
}

pub fn follow_office(shell: &mut Shell, office: Option<&(String, u64, bool)>) -> bool {
    use crate::fault::{Fault, Known};
    shell.office_trouble = None;
    let Some((key, moves, rotation)) = office else { return false };
    let Some((row, seat)) = shell.identities.as_ref().and_then(crate::identity::now_row) else { return false };
    let Some(target) = Address::parse(key) else { return false };
    let generation = row.office(seat).map(|o| o.generation).unwrap_or(0);
    if !*rotation || (row.current(seat) == Some(target) && generation == *moves) {
        return false;
    }
    let mut found: Option<crate::key::Secret> = None;
    let mut between: Vec<crate::key::Secret> = Vec::new();
    if row.kind() == crate::identity::Kind::Words {
        if let Ok(Some(seed)) = crate::keybox::get(&crate::places::seed_slot(&row.id)) {
            if let Ok(mut e) = <[u8; crate::family::ENTROPY_BYTES]>::try_from(seed.as_slice()) {
                let at = (0..=crate::family::GROUPS_SEARCHED).find(|g| crate::key::derived_at(&e, seat, *g).is_some_and(|k| k.address() == Some(target)));
                if let Some(at) = at {
                    found = crate::key::derived_at(&e, seat, at);
                    between = (1..at).filter_map(|g| crate::key::derived_at(&e, seat, g)).collect();
                }
                zikaron_os::wipe(&mut e);
            }
        }
    }
    if found.is_none() {
        found = crate::key::load_at(&crate::places::key_slot(&target)).ok().flatten();
    }
    let Some(k) = found else {
        shell.office_trouble = Some(Fault::known(Known::OfficeNotInWords, key.clone()));
        return false;
    };
    let slot = crate::places::key_slot(&target);
    if !crate::keybox::present(&slot).unwrap_or(false) {
        if let Err(f) = crate::key::install_at(&slot, &k) {
            shell.faults.push(f);
            return false;
        }
    }
    let mut held: Vec<Address> = Vec::new();
    for k in &between {
        let Some(a) = k.address() else { continue };
        let slot = crate::places::key_slot(&a);
        if crate::keybox::present(&slot).unwrap_or(false) || crate::key::install_at(&slot, k).is_ok() {
            held.push(a);
        }
    }
    let (rid, moves) = (row.id.clone(), *moves);
    match super::office::settle(shell, || {
        crate::register::change(|reg| {
            crate::identity::put_office(reg, &rid, seat, moves, target)?;
            crate::identity::hold_former(reg, &rid, seat, &held)?;
            Ok(reg.clone())
        })
    }) {
        Ok(()) => true,
        Err(f) => {
            shell.faults.push(f);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rotatex::{Book, Chain, Fell};

    #[test]
    fn a_transfer_is_gone_only_when_every_node_said_none_holds_it() {
        assert_eq!(out_of(crate::chainx::AsHeld::Nobody), Some(0));
        assert_eq!(out_of(crate::chainx::AsHeld::Unread), None);
        assert_eq!(out_of(crate::chainx::AsHeld::NoNode), None);
        assert_eq!(out_of(crate::chainx::AsHeld::Held(zikaron::json::Value::Null)), None);
    }

    #[test]
    fn a_step_that_went_in_stands_though_the_chain_is_not_read_after_it() {
        let next = crate::key::Address([0x22; 20]);
        let before = vec![Chain { chain: 7, book: Book::Read { pending: [0; 20], current: [0x11; 20], root: [0x11; 20], own: [0; 20] } }];
        let unread = crate::fault::Fault::known(crate::fault::Known::Unreachable, "node".to_string());
        let (chains, said) = after_send(Err(unread.clone()), before.clone(), true, 7, Fell::Declared { next });
        assert_eq!(chains[0].book, Book::Read { pending: next.0, current: [0x11; 20], root: [0x11; 20], own: next.0 }, "the declaration stands");
        assert_eq!(said, Some(unread.clone()), "the read's refusal said");
        let (chains, said) = after_send(Err(unread), before.clone(), false, 7, Fell::Declared { next });
        assert_eq!((chains, said.is_some()), (before, true), "one that did not go in: the reading from before, the refusal said");
    }
}
