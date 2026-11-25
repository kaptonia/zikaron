use super::*;

pub(super) fn batch(shell: &Shell, count: usize) -> Result<Batch, crate::fault::Fault> {
    if count == 0 {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::QueueEmpty,
            crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail029).to_string()),
        ));
    }
    let Some((chain, ids)) = shell.next_lane_here(count) else {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::QueueEmpty,
            crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail030).to_string()),
        ));
    };
    batch_on(shell, chain, ids)
}

fn batch_on(shell: &Shell, chain: u64, ids: Vec<String>) -> Result<Batch, crate::fault::Fault> {
    let hashes = crate::queue::Queue::hashes(&ids)?;
    shell.settings.chain_id.ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoChainId, String::new())
    })?;
    let registry = registry_on(shell, chain)?;
    let urls: Vec<crate::chainx::NodeAddr> = shell.endpoints.iter().filter(|e| e.chain == chain).map(|e| e.url.clone()).collect();
    if urls.is_empty() {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::NoEndpoint,
            crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail031, &[&(chain).to_string()])),
        ));
    }
    let carried = carried_here(shell, &ids, chain);
    if let (Some(_), crate::facex::State::Read(b)) = (&carried, &shell.persona) {
        if let Err(barred) = zikaron_glue::facegate::sealed(&b.table) {
            return Err(super::face::barred(barred));
        }
    }
    Ok(Batch { ids, hashes, chain, registry: registry.0, urls, backoff: shell.send_backoff.clone(), basis: shell.lanes(), carried })
}

pub(super) fn carried_here(shell: &Shell, ids: &[String], chain: u64) -> Option<Vec<u8>> {
    let root = shell.home.as_ref()?.root().to_path_buf();
    carried_from(&root, &carry_of(shell, ids, chain), ids, chain)
}

pub(super) enum Carry {
    Nothing,
    Face(String),
    Unplaced,
}

pub(super) fn carry_of(shell: &Shell, ids: &[String], chain: u64) -> Carry {
    let [id] = ids else { return Carry::Nothing };
    match shell.carried_on(id) {
        Some(Some(c)) if c == chain => Carry::Face(id.to_ascii_lowercase()),
        Some(_) => Carry::Nothing,
        None => Carry::Unplaced,
    }
}

pub(super) fn carried_from(root: &std::path::Path, carry: &Carry, ids: &[String], chain: u64) -> Option<Vec<u8>> {
    let pile = || crate::home::Home::open(root).ok().and_then(|h| h.ledger().ok()).and_then(|l| l.pile().ok()).map(|p| p.items);
    match carry {
        Carry::Nothing => None,
        Carry::Face(id) => pile()?.into_iter().find(|b| zikaron::hexfmt::encode(&zikaron::cryptox::sha256(b)) == *id),
        Carry::Unplaced => zikaron_send::batch::carried_of(&pile()?, ids, chain),
    }
}

pub(super) struct GasAsk {
    count: usize,
    ask: zikaron_send::batch::GasAsk,
    lane: (u64, Vec<String>),
}

pub(super) fn gas_ask(shell: &Shell, count: usize) -> Result<GasAsk, crate::fault::Fault> {
    let b = batch(shell, count)?;
    let who = shell.anchor.ok_or_else(|| {
        crate::fault::Fault::known(
            crate::fault::Known::KeychainMissing,
            crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail032).to_string()),
        )
    })?;
    let data = b.calldata();
    let call = zikaron_anchor::send::estimate_call(&who.0, &b.registry, &data);
    let eps: Vec<crate::chainx::Endpoint> = shell
        .endpoints
        .iter()
        .filter(|e| e.chain == b.chain)
        .cloned()
        .collect();
    Ok(GasAsk { count, lane: (b.chain, b.ids.clone()), ask: zikaron_send::batch::GasAsk { chain: b.chain, call, data, eps } })
}

pub(super) fn estimate_on(a: GasAsk) -> Result<crate::task::Done, crate::fault::Fault> {
    crate::chainx::speak();
    let e = zikaron_send::batch::estimate(&a.ask)?;
    let fees_left = e.fees_left.iter().map(crate::chainx::said_of).collect();
    Ok(crate::task::Done::Gas { count: a.count, gas: e.gas, calldata: e.calldata, fees: e.fees, fees_left, head_time: e.head_time, lane: a.lane })
}

pub(super) fn gas_shown(shell: &Shell, count: usize) -> Result<u64, crate::fault::Fault> {
    match shell.gas {
        Some((n, g)) if n == count => Ok(g),
        _ => Err(crate::fault::Fault::known(
            crate::fault::Known::GasNotShown,
            crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail035, &[&(count).to_string()])),
        )),
    }
}

pub(super) fn send_batch(shell: &mut Shell, count: usize) -> Result<Spawned, crate::fault::Fault> {
    let gas = Some(gas_shown(shell, count)?);
    shell.may_send_entries()?;
    let b = match shell.gas_lane.clone() {
        Some((chain, ids)) if ids.len() == count && ids.iter().all(|id| shell.queue.step_on(id, chain).is_some_and(|s| !s.in_flight())) => batch_on(shell, chain, ids)?,
        _ => batch(shell, count)?,
    };
    let secret = signing_key(shell, crate::sign::Use::Anchor)?;
    let root = shell
        .home
        .as_ref()
        .map(|h| h.root().to_path_buf())
        .ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    let fees = shell.fees.unwrap_or_else(zikaron_anchor::send::Fees::fallback);
    let ask = crate::exitgate::ask_of(shell)?;
    Ok(shell.tasks.spawn(Kind::Anchor, move || anchor_batch(b, secret, root, gas, fees, ask)))
}

pub fn resume(shell: &mut Shell) -> bool {
    shell.resume_blocked = None;
    if shell.tasks.in_flight(Kind::Anchor) || !shell.may_send() {
        return false;
    }
    let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) else { return false };
    let Some((txs, chain, ids)) = zikaron_send::batch::out_first(&shell.queue) else { return false };
    let urls = receipt_urls(shell, chain, None);
    if urls.is_empty() {
        shell.resume_blocked = Some(chain);
        return false;
    }
    let wait = shell.anchor_wait;
    let backoff = shell.receipt_backoff.clone();
    let (sender, nonce) = (sender_of(shell, chain), batch_nonce(shell, &ids));
    let hand = shell.lock.as_ref().map(crate::lock::Lock::hand);
    let carry = carry_of(shell, &ids, chain);
    matches!(
        shell.tasks.spawn_turnless(Kind::Anchor, move || {
            let carried = carried_from(&root, &carry, &ids, chain);
            confirm_batch(hand, root, urls, chain, txs, ids, None, wait, &backoff, sender, nonce, carried)
        }),
        Spawned::Started
    )
}

fn registry_on(shell: &Shell, chain: u64) -> Result<crate::key::Address, crate::fault::Fault> {
    shell.basis_chains().into_iter().find(|(c, _, _)| *c == chain).map(|(_, r, _)| r).ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoRegistry, String::new()))
}

fn sender_of(shell: &Shell, chain: u64) -> Option<zikaron_send::batch::Sender> {
    Some(zikaron_send::batch::Sender { from: shell.anchor?.0, registry: registry_on(shell, chain).ok()?.0 })
}

fn batch_nonce(shell: &Shell, ids: &[String]) -> Option<u64> {
    ids.first().and_then(|id| shell.queue.step_of(id)).and_then(crate::queue::Step::nonce)
}

pub(super) fn catch_up(shell: &mut Shell) -> Applied {
    let audit = shell.audit_stale() && matches!(start_audit(shell), Ok(Spawned::Started));
    let review = shell.home.is_some()
        && !shell.endpoints.is_empty()
        && shell.settings.chain_id.is_some()
        && shell.settings.registry.is_some()
        && matches!(review(shell), Ok(Spawned::Started));
    let resume = resume(shell);
    Applied::CaughtUp { audit, review, resume }
}

pub fn audit_after_send(shell: &mut Shell, landed: &Done) {
    if let Done::Anchored { tx, chain, state, sent, kept, .. } = landed {
        if let Some((1, block)) = receipt_of(state) {
            if super::audit::merge_sent(shell, *chain, tx, block, *sent, kept.as_ref()) {
                return;
            }
        }
    }
    if let Err(f) = start_audit(shell) {
        shell.faults.push(f);
    }
}

pub fn wait_submitted(shell: &mut Shell, tx: String, chain: u64, url: crate::chainx::NodeAddr, ids: Vec<String>, gas: Option<u64>) {
    let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) else { return };
    let urls = receipt_urls(shell, chain, Some(&url));
    let wait = shell.anchor_wait;
    let backoff = shell.receipt_backoff.clone();
    let txs = ids.first().and_then(|id| shell.queue.step_of(id)).and_then(|s| s.awaited()).map(|(t, _)| t).unwrap_or_else(|| vec![tx]);
    let (sender, nonce) = (sender_of(shell, chain), batch_nonce(shell, &ids));
    let hand = shell.lock.as_ref().map(crate::lock::Lock::hand);
    let carry = carry_of(shell, &ids, chain);
    let _ = shell.tasks.spawn_turnless(Kind::Anchor, move || {
        let carried = carried_from(&root, &carry, &ids, chain);
        confirm_batch(hand, root, urls, chain, txs, ids, gas, wait, &backoff, sender, nonce, carried)
    });
}

pub(super) fn receipt_urls(shell: &Shell, chain: u64, taken: Option<&crate::chainx::NodeAddr>) -> Vec<crate::chainx::NodeAddr> {
    let mut out: Vec<crate::chainx::NodeAddr> = taken.cloned().into_iter().collect();
    for e in shell.endpoints.iter().filter(|e| e.chain == chain) {
        if !out.iter().any(|u| *u == e.url) {
            out.push(e.url.clone());
        }
    }
    out
}

pub fn funds_check(need: u128, have: u128) -> Result<(), crate::fault::Fault> {
    Ok(zikaron_send::batch::funds_check(need, have)?)
}

pub fn receipt_state(status: u64, block: u64) -> String {
    format!("status {status} · block {block}")
}

pub fn receipt_of(state: &str) -> Option<(u64, u64)> {
    let (status, block) = state.strip_prefix("status ")?.split_once(" · block ")?;
    Some((status.parse().ok()?, block.parse().ok()?))
}

fn unheard_state(said: &str) -> String {
    format!("unreachable: {said}")
}

pub fn receipt_unheard(state: &str) -> bool {
    state.starts_with(&unheard_state(""))
}

pub fn receipt_failed(state: &str) -> bool {
    state
        .strip_prefix("status ")
        .and_then(|x| x.split_whitespace().next())
        .map(|x| x != "1")
        .unwrap_or(false)
}

pub fn funds_tail(need: u128, have: u128) -> String {
    format!("need={need} have={have}")
}

pub fn funds_tail_of(need: u128, have: u128, key: &crate::key::Address) -> String {
    format!("{} {}", funds_tail(need, have), funds_key_word(key))
}

pub fn funds_key_word(key: &crate::key::Address) -> String {
    format!("key={}", key.hex())
}

pub fn funds_key(tail: &str) -> Option<crate::key::Address> {
    tail.split_whitespace().find_map(|x| x.strip_prefix("key=")).and_then(crate::key::Address::parse)
}

pub fn funds_of(tail: &str) -> Option<(u128, u128)> {
    let need = tail.split_whitespace().find_map(|x| x.strip_prefix("need="))?.parse().ok()?;
    let have = tail.split_whitespace().find_map(|x| x.strip_prefix("have="))?.parse().ok()?;
    Some((need, have))
}

pub(super) fn anchor_batch(
    b: Batch,
    secret: crate::key::Secret,
    root: std::path::PathBuf,
    gas: Option<u64>,
    fees: zikaron_anchor::send::Fees,
    ask: crate::exitgate::Ask,
) -> Result<Done, crate::fault::Fault> {
    let desk = crate::sendx::Desk::open(&root)?;
    let ids = b.ids.clone();
    zikaron_send::batch::anchor(&desk, b, &secret, gas, fees, ask).map(submitted).map_err(|f| {
        match zikaron_send::keep::read(&desk).ok().and_then(|q| zikaron_send::batch::out_for(&q, &ids)).and_then(|(txs, _, _)| txs.last().cloned()) {
            Some(tx) => f.sent_out(tx),
            None => f,
        }
    })
}

fn submitted(s: zikaron_send::batch::Sent) -> Done {
    Done::Submitted { tx: s.tx, chain: s.chain, url: s.url, ids: s.ids, gas: s.gas, queue: s.queue }
}

pub(super) fn bump_batch(shell: &mut Shell, tx: &str, cap: u64) -> Result<Spawned, crate::fault::Fault> {
    shell.may_send_entries()?;
    let not_shown = |why: &str| crate::fault::Fault::known(crate::fault::Known::GasNotShown, format!("{tx} · {why}"));
    let stuck = shell.stuck.clone().filter(|s| s.txs.last().map(String::as_str) == Some(tx)).ok_or_else(|| not_shown("no resend offered for this transaction"))?;
    let fees = match stuck.offer {
        crate::task::Offer::Resend { fees, .. } if fees.max_fee <= cap => fees,
        crate::task::Offer::Unheld { fees, .. } if fees.max_fee <= cap => fees,
        crate::task::Offer::Resend { fees, .. } | crate::task::Offer::Unheld { fees, .. } => return Err(not_shown(&format!("the fees offered are now {} wei per gas, above the {cap} shown", fees.max_fee))),
        crate::task::Offer::NotAbove { .. } => return Err(not_shown("the price now is not above its cap")),
        crate::task::Offer::PriceUnread => return Err(not_shown("the price now was not read")),
        crate::task::Offer::Spent => return Err(not_shown(&format!("resent {} times", crate::queue::RESENDS_MAX))),
    };
    let ids: Vec<String> = shell.queue.items.iter().filter(|q| q.step.awaited().is_some_and(|(t, c)| t == stuck.txs && c == stuck.chain)).map(|q| q.id.clone()).collect();
    if ids.is_empty() {
        return Err(crate::fault::Fault::known(crate::fault::Known::QueueEmpty, tx.to_string()));
    }
    let registry = registry_on(shell, stuck.chain)?;
    let urls = receipt_urls(shell, stuck.chain, None);
    if urls.is_empty() {
        return Err(crate::fault::Fault::known(crate::fault::Known::NoEndpoint, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail031, &[&stuck.chain.to_string()]))));
    }
    let b = Batch { hashes: crate::queue::Queue::hashes(&ids)?, ids, chain: stuck.chain, registry: registry.0, urls, backoff: shell.send_backoff.clone(), basis: shell.lanes(), carried: None };
    let secret = signing_key(shell, crate::sign::Use::Anchor)?;
    let root = shell.home.as_ref().map(|h| h.root().to_path_buf()).ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    let ask = crate::exitgate::ask_of(shell)?;
    Ok(shell.tasks.spawn(Kind::Anchor, move || resend_batch(b, stuck, fees, secret, root, ask)))
}

fn resend_batch(b: Batch, stuck: crate::task::Stuck, fees: zikaron_anchor::send::Fees, secret: crate::key::Secret, root: std::path::PathBuf, ask: crate::exitgate::Ask) -> Result<Done, crate::fault::Fault> {
    let desk = crate::sendx::Desk::open(&root)?;
    zikaron_send::batch::resend(&desk, b, stuck, fees, &secret, ask).map(submitted)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn confirm_batch(
    hand: Option<crate::lock::Hand>,
    root: std::path::PathBuf,
    urls: Vec<crate::chainx::NodeAddr>,
    chain: u64,
    txs: Vec<String>,
    ids: Vec<String>,
    gas: Option<u64>,
    wait: std::time::Duration,
    backoff: &[std::time::Duration],
    sender: Option<zikaron_send::batch::Sender>,
    nonce: Option<u64>,
    carried: Option<Vec<u8>>,
) -> Result<Done, crate::fault::Fault> {
    let mut d = anchored(settled_wait(hand, &root, urls, chain, txs, ids, gas, wait, backoff, sender, nonce, carried)?);
    if let Done::Anchored { whole, .. } = &mut d {
        use crate::queue::Kept as _;
        let at = crate::reads::ticket(crate::reads::Copies::Queue);
        *whole = Some((crate::queue::Queue::read(&crate::home::Home::bare(root.clone())), at));
    }
    Ok(d)
}

#[allow(clippy::too_many_arguments)]
fn settled_wait(
    hand: Option<crate::lock::Hand>,
    root: &std::path::Path,
    urls: Vec<crate::chainx::NodeAddr>,
    chain: u64,
    txs: Vec<String>,
    ids: Vec<String>,
    gas: Option<u64>,
    wait: std::time::Duration,
    backoff: &[std::time::Duration],
    sender: Option<zikaron_send::batch::Sender>,
    nonce: Option<u64>,
    carried: Option<Vec<u8>>,
) -> Result<zikaron_send::batch::Landed, crate::fault::Fault> {
    let desk = crate::sendx::Desk::open(root)?;
    let out = |f: crate::fault::Fault| f.sent_out(txs.last().cloned().unwrap_or_default());
    let mut heard = zikaron_send::batch::wait(urls, chain, txs.clone(), ids, gas, wait, backoff, sender, nonce, carried.as_deref()).map_err(|t| out(t.into()))?;
    zikaron_send::batch::landed_facts(&desk, &mut heard);
    crate::lock::settling(hand.as_ref(), root, || zikaron_send::batch::settle(&desk, heard)).map_err(out)?.map_err(out)
}

fn anchored(l: zikaron_send::batch::Landed) -> Done {
    Done::Anchored {
        tx: l.tx,
        chain: l.chain,
        confirmed: l.confirmed,
        state: state_of(&l.receipt),
        sent: l.sent,
        dropped: l.dropped,
        queue: l.queue,
        gas: l.gas,
        calldata: l.calldata,
        stuck: l.stuck,
        voided: l.voided,
        waited: l.waited,
        kept: l.kept,
        whole: None,
    }
}

pub fn await_out(shell: &mut Shell) -> Result<Spawned, crate::fault::Fault> {
    if let Some(s) = shell.unfetched.as_ref() {
        return Err(crate::restorex::fault(s));
    }
    let Some((txs, chain, ids)) = zikaron_send::batch::out_first(&shell.queue) else {
        return Err(crate::fault::Fault::known(crate::fault::Known::QueueEmpty, String::new()));
    };
    let root = shell.home.as_ref().map(|h| h.root().to_path_buf()).ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    let urls = receipt_urls(shell, chain, None);
    if urls.is_empty() {
        return Err(crate::fault::Fault::known(crate::fault::Known::NoEndpoint, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail031, &[&chain.to_string()]))));
    }
    let switch = shell.machine.cli_anchor;
    let again = match switch {
        zikaron_seal::machine::CliAnchor::Send => {
            let registry = registry_on(shell, chain)?;
            let b = Batch { hashes: crate::queue::Queue::hashes(&ids)?, ids: ids.clone(), chain, registry: registry.0, urls: urls.clone(), backoff: shell.send_backoff.clone(), basis: shell.lanes(), carried: None };
            Some((b, signing_key(shell, crate::sign::Use::Anchor)?, crate::exitgate::ask_of(shell)?))
        }
        _ => None,
    };
    let wait = shell.anchor_wait;
    let backoff = shell.receipt_backoff.clone();
    let (sender, nonce) = (sender_of(shell, chain), batch_nonce(shell, &ids));
    let hand = shell.lock.as_ref().map(crate::lock::Lock::hand);
    let carry = carry_of(shell, &ids, chain);
    Ok(shell.tasks.spawn_turnless(Kind::Anchor, move || {
        let carried = carried_from(&root, &carry, &ids, chain);
        let l = settled_wait(hand, &root, urls, chain, txs, ids.clone(), None, wait, &backoff, sender, nonce, carried)?;
        match (zikaron_send::batch::again_at(&l, switch), again) {
            (Some((stuck, fees)), Some((b, secret, ask))) if zikaron_send::batch::out_for(&zikaron_send::keep::read(&crate::sendx::Desk::open(&root)?)?, &ids).map(|(t, _, _)| t).as_ref() == Some(&stuck.txs) => {
                resend_batch(b, stuck, fees, secret, root, ask)
            }
            _ => Ok(anchored(l)),
        }
    }))
}

fn state_of(r: &zikaron_send::batch::Receipt) -> String {
    use zikaron_send::batch::Receipt;
    match r {
        Receipt::Included { status, block } => receipt_state(*status, *block),
        Receipt::NotYet => "not yet in a block".to_string(),
        Receipt::Unheard(said) => unheard_state(said),
        Receipt::Split { has, not_yet } => format!("receipt at {} only; not yet at {}", has.join(" "), not_yet.join(" ")),
        Receipt::Void => "void: held by no node, its nonce used by another transaction".to_string(),
        Receipt::Unused => "held by no node; its nonce unused".to_string(),
    }
}
