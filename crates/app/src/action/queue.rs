use super::*;

/// The batch of `count` sendable entries from the shell's copy of the queue, with this home's chain, registry and
/// that chain's nodes (refused by name when any is missing).
pub(super) fn batch(shell: &Shell, count: usize) -> Result<Batch, crate::fault::Fault> {
    if count == 0 {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::QueueEmpty,
            crate::lang::t(crate::lang::Key::Tail029).to_string(),
        ));
    }
    let ids = shell.queue.take_ids(count);
    if ids.is_empty() {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::QueueEmpty,
            crate::lang::t(crate::lang::Key::Tail030).to_string(),
        ));
    }
    let hashes = crate::queue::Queue::hashes(&ids)?;
    let chain = shell.settings.chain_id.ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoChainId, String::new())
    })?;
    let registry = shell.settings.registry.ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoRegistry, String::new())
    })?;
    let urls: Vec<crate::chainx::NodeAddr> = shell.endpoints.iter().filter(|e| e.chain == chain).map(|e| e.url.clone()).collect();
    if urls.is_empty() {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::NoEndpoint,
            crate::lang::filln(crate::lang::Key::Tail031, &[&(chain).to_string()]),
        ));
    }
    Ok(Batch { ids, hashes, chain, registry: registry.0, urls, backoff: shell.send_backoff.clone() })
}

/// What estimating a batch needs, read on the UI thread without any network call: the batch size and the
/// sender's question (its chain, the call to estimate, byte-identical to the real transaction, and that chain's
/// nodes).
pub(super) struct GasAsk {
    count: usize,
    ask: zikaron_send::batch::GasAsk,
}

/// The UI-thread half of gas estimation: the batch, the anchor key's address and the nodes. Nothing here
/// touches the network; [`estimate_on`] does, on the task's thread.
pub(super) fn gas_ask(shell: &Shell, count: usize) -> Result<GasAsk, crate::fault::Fault> {
    let b = batch(shell, count)?;
    let who = shell.anchor.ok_or_else(|| {
        crate::fault::Fault::known(
            crate::fault::Known::KeychainMissing,
            crate::lang::t(crate::lang::Key::Tail032).to_string(),
        )
    })?;
    let data = zikaron_anchor::send::registry_calldata(&b.hashes);
    let call = zikaron_anchor::send::estimate_call(&who.0, &b.registry, &data);
    let eps: Vec<crate::chainx::Endpoint> = shell
        .endpoints
        .iter()
        .filter(|e| e.chain == b.chain)
        .cloned()
        .collect();
    Ok(GasAsk { count, ask: zikaron_send::batch::GasAsk { chain: b.chain, call, data, eps } })
}

/// Estimates gas once on the task's thread, by the sender's one rule (`zikaron_send::batch::estimate`, the command
/// line's too), through this side's endpoints; the estimate lands as `Done::Gas` with the nodes left out said here.
pub(super) fn estimate_on(a: GasAsk) -> Result<crate::task::Done, crate::fault::Fault> {
    crate::chainx::speak();
    let e = zikaron_send::batch::estimate(&a.ask)?;
    let fees_left = e.fees_left.iter().map(crate::chainx::said_of).collect();
    Ok(crate::task::Done::Gas { count: a.count, gas: e.gas, calldata: e.calldata, fees: e.fees, fees_left, head_time: e.head_time })
}

/// The "show before send" check: whether gas was estimated for this batch, at this count.
///
/// Kept separate so sending can only be reached through it.
pub(super) fn gas_shown(shell: &Shell, count: usize) -> Result<u64, crate::fault::Fault> {
    match shell.gas {
        Some((n, g)) if n == count => Ok(g),
        _ => Err(crate::fault::Fault::known(
            crate::fault::Known::GasNotShown,
            crate::lang::filln(crate::lang::Key::Tail035, &[&(count).to_string()]),
        )),
    }
}

pub(super) fn send_batch(shell: &mut Shell, count: usize) -> Result<Spawned, crate::fault::Fault> {
    // Show before send: gas must have been estimated for this exact batch size.
    let gas = Some(gas_shown(shell, count)?);
    // Anchoring does not require the ledger to be writable: anchoring hashes of recorded bytes appends
    // nothing, and the succession entry itself must be anchorable, or the new key can never verify it and the
    // handover stalls. So the handed-over refusal is let through while the others (lock, broken chain, pen)
    // still block, and entries queued before a handover can still reach the chain.
    if let Err(f) = shell.may_write_entries() {
        if !f.said().starts_with("HANDED_OVER") {
            return Err(f);
        }
    }
    let b = batch(shell, count)?;
    let secret = signing_key(shell, crate::sign::Use::Anchor)?;
    let root = shell
        .home
        .as_ref()
        .map(|h| h.root().to_path_buf())
        .ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    // The private key bytes never enter this layer: the `Secret` is moved whole into the background task, lent
    // only through its signing port (`sign`, the sender's `Signer`) for exactly one signature, and zeroed when
    // dropped. The fee reading comes
    // with the estimate; tests that set gas directly without estimating use `Fees::fallback`.
    let fees = shell.fees.unwrap_or_else(zikaron_anchor::send::Fees::fallback);
    // What the exit gate needs is taken here; the chain is read in the task as its last step before sending.
    let ask = crate::exitgate::ask_of(shell)?;
    // Whether the chain already holds these anchors is asked from this home's scan window, as the gate reads it.
    let start = shell.settings.from_block;
    Ok(shell.tasks.spawn(Kind::Anchor, move || anchor_batch(b, secret, root, gas, fees, ask, start)))
}

/// Resumes waiting for submitted transactions: entries the queue file records as submitted without a receipt
/// get a task that only waits for the receipt, without resending. Called on home opening and by the window's
/// periodic check; skipped while an anchor task is in flight. Returns whether a task started.
pub fn resume(shell: &mut Shell) -> bool {
    // This flag only means "cannot resume now". It is cleared on every call so a stale reason never lingers
    // (a changed home, a settled batch or a node added back should clear it at once).
    shell.resume_blocked = None;
    if shell.tasks.in_flight(Kind::Anchor) || !shell.writable() {
        return false;
    }
    let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) else { return false };
    let Some((txs, chain, ids)) = shell.queue.submitted().into_iter().next() else { return false };
    let urls = receipt_urls(shell, chain, None);
    if urls.is_empty() {
        // A failed resume must be visible: a submitted batch is waiting for a receipt, but the current
        // settings have no node for its chain (basis changed, node removed). Silently returning false would
        // retry every fifteen seconds while those entries wait forever and never join the next batch. It is
        // recorded on the shell and the queue page shows it.
        shell.resume_blocked = Some(chain);
        return false;
    }
    let wait = shell.anchor_wait;
    let backoff = shell.receipt_backoff.clone();
    let (sender, nonce) = (sender_of(shell), batch_nonce(shell, &ids));
    let hand = shell.lock.as_ref().map(crate::lock::Lock::hand);
    matches!(
        shell.tasks.spawn_turnless(Kind::Anchor, move || confirm_batch(hand, root, urls, chain, txs, ids, None, wait, &backoff, sender, nonce)),
        Spawned::Started
    )
}

/// The account a batch is sent from and the registry it calls, read on the UI thread. Used to judge a batch no
/// node holds (via the account's on-chain nonce) and to resend it (`confirm_batch`). `None` if either is
/// missing.
fn sender_of(shell: &Shell) -> Option<zikaron_send::batch::Sender> {
    Some(zikaron_send::batch::Sender { from: shell.anchor?.0, registry: shell.settings.registry?.0 })
}

/// The nonce the queue records for these entries' batch (`None` in files written by versions that did not
/// record it).
fn batch_nonce(shell: &Shell, ids: &[String]) -> Option<u64> {
    ids.first().and_then(|id| shell.queue.step_of(id)).and_then(crate::queue::Step::nonce)
}

/// Runs once right after unlocking to catch up on what was missed while locked: the self-audit (raising missed
/// notices once each), the vault review with its sentinel, and the receipt wait of a submitted batch. Each
/// starts only where it can (a home with a basis and nodes); nothing is reported for what cannot start.
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

/// Runs one self-audit right after a batch is confirmed on chain (its label moves the pen as usual). If it
/// cannot start (no basis or nodes), the audit's own refusal is reported.
pub fn audit_after_send(shell: &mut Shell) {
    if let Err(f) = start_audit(shell) {
        shell.faults.push(f);
    }
}

/// After a broadcast lands, waits for the receipt (called where the shell receives task results).
pub fn wait_submitted(shell: &mut Shell, tx: String, chain: u64, url: crate::chainx::NodeAddr, ids: Vec<String>, gas: Option<u64>) {
    let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) else { return };
    let urls = receipt_urls(shell, chain, Some(&url));
    let wait = shell.anchor_wait;
    let backoff = shell.receipt_backoff.clone();
    // Await every transaction of the batch: the one just sent and those it replaces (the queue file records
    // them all).
    let txs = ids.first().and_then(|id| shell.queue.step_of(id)).and_then(|s| s.awaited()).map(|(t, _)| t).unwrap_or_else(|| vec![tx]);
    let (sender, nonce) = (sender_of(shell), batch_nonce(shell, &ids));
    let hand = shell.lock.as_ref().map(crate::lock::Lock::hand);
    let _ = shell.tasks.spawn_turnless(Kind::Anchor, move || confirm_batch(hand, root, urls, chain, txs, ids, gas, wait, &backoff, sender, nonce));
}

/// Where to ask for the receipt: the endpoint that accepted the transaction first, then this chain's other
/// configured endpoints (deduplicated).
pub(super) fn receipt_urls(shell: &Shell, chain: u64, taken: Option<&crate::chainx::NodeAddr>) -> Vec<crate::chainx::NodeAddr> {
    let mut out: Vec<crate::chainx::NodeAddr> = taken.cloned().into_iter().collect();
    for e in shell.endpoints.iter().filter(|e| e.chain == chain) {
        if !out.iter().any(|u| *u == e.url) {
            out.push(e.url.clone());
        }
    }
    out
}

/// The balance gate's comparison (`zikaron_send::batch::funds_check`): a balance at least equal to the maximum
/// spend passes (exactly enough included); one wei less is refused by name with both numbers ([`funds_tail`]).
pub fn funds_check(need: u128, have: u128) -> Result<(), crate::fault::Fault> {
    Ok(zikaron_send::batch::funds_check(need, have)?)
}

/// Formats the receipt state (`Done::Anchored.state` is written and parsed only here).
pub fn receipt_state(status: u64, block: u64) -> String {
    format!("status {status} · block {block}")
}

/// Parses the receipt state back into status and block, if it describes a receipt ([`receipt_state`]).
pub fn receipt_of(state: &str) -> Option<(u64, u64)> {
    let (status, block) = state.strip_prefix("status ")?.split_once(" · block ")?;
    Some((status.parse().ok()?, block.parse().ok()?))
}

/// The state written when no node answered the receipt query (parsed by [`receipt_unheard`]).
fn unheard_state(said: &str) -> String {
    format!("unreachable: {said}")
}

/// Whether the receipt state says no node answered ([`unheard_state`]).
pub fn receipt_unheard(state: &str) -> bool {
    state.starts_with(&unheard_state(""))
}

/// Whether the receipt status is not 1 (included but execution failed, which is not an anchor). Only this case
/// keeps the plain wording of `SEND_FAILED`.
pub fn receipt_failed(state: &str) -> bool {
    state
        .strip_prefix("status ")
        .and_then(|x| x.split_whitespace().next())
        .map(|x| x != "1")
        .unwrap_or(false)
}

/// Formats the balance gate's evidence tail (written and parsed only here).
pub fn funds_tail(need: u128, have: u128) -> String {
    format!("need={need} have={have}")
}

/// Parses the required and current amounts from the evidence tail (`None` if absent, as in a node refusal,
/// whose message lacks the two numbers).
pub fn funds_of(tail: &str) -> Option<(u128, u128)> {
    let need = tail.split_whitespace().find_map(|x| x.strip_prefix("need="))?.parse().ok()?;
    let have = tail.split_whitespace().find_map(|x| x.strip_prefix("have="))?.parse().ok()?;
    Some((need, have))
}

/// Hands this batch to the sender (`zikaron_send::batch::anchor`) for this home, on the task's thread: earlier
/// transactions of these entries asked about by hash, the chain asked whether it holds them already, the balance
/// and exit gates, the signature through this desktop's key (`sign`), the batch landed as submitted before its
/// bytes go, and the broadcast. It goes only as far as "submitted": `Done::Submitted` is returned and the shell
/// starts the receipt wait (`confirm_batch`).
pub(super) fn anchor_batch(
    b: Batch,
    secret: crate::key::Secret,
    root: std::path::PathBuf,
    gas: Option<u64>,
    fees: zikaron_anchor::send::Fees,
    ask: crate::exitgate::Ask,
    start: u64,
) -> Result<Done, crate::fault::Fault> {
    let desk = crate::sendx::Desk::open(&root)?;
    let ids = b.ids.clone();
    zikaron_send::batch::anchor(&desk, b, &secret, gas, fees, ask, start).map(submitted).map_err(|f| {
        // Failed after the batch was landed in flight (no node answered the broadcast; its bytes may be in a pool):
        // the batch is out, awaited by its hash, and the refusal carries that transaction (`Fault::out`).
        match zikaron_send::keep::read(&desk).ok().and_then(|q| zikaron_send::batch::out_for(&q, &ids)).and_then(|(txs, _, _)| txs.last().cloned()) {
            Some(tx) => f.sent_out(tx),
            None => f,
        }
    })
}

/// A batch the sender put out (or found out already), as the shell receives it.
fn submitted(s: zikaron_send::batch::Sent) -> Done {
    Done::Submitted { tx: s.tx, chain: s.chain, url: s.url, ids: s.ids, gas: s.gas, queue: s.queue }
}

/// Resends a stuck batch with higher fees, on the person's request: only the batch whose last transaction is
/// `tx`, only when the last receipt wait offered a resend (`Shell::stuck`, `Offer::Resend`), and never above the
/// fee cap shown on the card (`cap`). If the offer has since risen above it, the resend is refused by name (the
/// card shows the new figure), so the cost never exceeds what was seen. Every reason a resend is not possible is
/// named as `GAS_NOT_SHOWN` (the resend's show-before-send gate): no offer for this transaction, the price not
/// above its cap, the price unread, or already resent `queue::RESENDS_MAX` times.
pub(super) fn bump_batch(shell: &mut Shell, tx: &str, cap: u64) -> Result<Spawned, crate::fault::Fault> {
    if let Err(f) = shell.may_write_entries() {
        if !f.said().starts_with("HANDED_OVER") {
            return Err(f);
        }
    }
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
    let registry = shell.settings.registry.ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoRegistry, String::new()))?;
    let urls = receipt_urls(shell, stuck.chain, None);
    if urls.is_empty() {
        return Err(crate::fault::Fault::known(crate::fault::Known::NoEndpoint, crate::lang::filln(crate::lang::Key::Tail031, &[&stuck.chain.to_string()])));
    }
    let b = Batch { hashes: crate::queue::Queue::hashes(&ids)?, ids, chain: stuck.chain, registry: registry.0, urls, backoff: shell.send_backoff.clone() };
    let secret = signing_key(shell, crate::sign::Use::Anchor)?;
    let root = shell.home.as_ref().map(|h| h.root().to_path_buf()).ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    let ask = crate::exitgate::ask_of(shell)?;
    let start = shell.settings.from_block;
    Ok(shell.tasks.spawn(Kind::Anchor, move || resend_batch(b, stuck, fees, secret, root, ask, start)))
}

/// The resend task, the sender's (`zikaron_send::batch::resend`): the chain asked whether it holds these anchors,
/// the balance for the new cap, the exit gate, the same key signing the same transaction again, the new hash
/// recorded with the batch before the single broadcast.
fn resend_batch(b: Batch, stuck: crate::task::Stuck, fees: zikaron_anchor::send::Fees, secret: crate::key::Secret, root: std::path::PathBuf, ask: crate::exitgate::Ask, start: u64) -> Result<Done, crate::fault::Fault> {
    let desk = crate::sendx::Desk::open(&root)?;
    zikaron_send::batch::resend(&desk, b, stuck, fees, &secret, ask, start).map(submitted)
}

/// Waits at most `wait` for the receipt, holding no lock (the sender's receipt wait, `zikaron_send::batch::wait`):
/// included, reverted, not yet, held by no node and judged by its nonce (void and requeued, or offered again);
/// then settles what it heard under the home's writer lock (`zikaron_send::batch::settle` through
/// `lock::settling`: the shell's, or taken for that one write while the command line writes alone). The receipt
/// state is written here ([`state_of`]).
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
) -> Result<Done, crate::fault::Fault> {
    let desk = crate::sendx::Desk::open(&root)?;
    // This batch is out: a wait or a settling that fails says so with its transaction (`Fault::out`), never as
    // nothing sent.
    let out = |f: crate::fault::Fault| f.sent_out(txs.last().cloned().unwrap_or_default());
    let heard = zikaron_send::batch::wait(urls, chain, txs.clone(), ids, gas, wait, backoff, sender, nonce).map_err(|t| out(t.into()))?;
    let l = crate::lock::settling(hand.as_ref(), &root, || zikaron_send::batch::settle(&desk, heard)).map_err(out)?.map_err(out)?;
    Ok(Done::Anchored {
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
    })
}

/// The receipt state as `Done::Anchored.state` carries it (parsed back by [`receipt_of`], [`receipt_unheard`],
/// [`receipt_failed`]).
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
