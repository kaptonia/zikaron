use super::*;

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
    Ok(Batch { ids, hashes, chain, registry, urls, backoff: shell.send_backoff.clone() })
}

/// What estimating a batch needs, read on the UI thread without any network call: the batch size, its chain,
/// the call to estimate (byte-identical to the real transaction, both built by the anchoring crate's
/// `registry_calldata`) and that chain's nodes.
pub(super) struct GasAsk {
    count: usize,
    chain: u64,
    call: zikaron::json::Value,
    data: Vec<u8>,
    eps: Vec<crate::chainx::Endpoint>,
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
    let call = zikaron_anchor::send::estimate_call(&who.0, &b.registry.0, &data);
    let eps: Vec<crate::chainx::Endpoint> = shell
        .endpoints
        .iter()
        .filter(|e| e.chain == b.chain)
        .cloned()
        .collect();
    Ok(GasAsk { count, chain: b.chain, call, data, eps })
}

/// Estimates gas once on the task's thread, using the same rule as the command line
/// (`zikaron_anchor::send::estimate_gas`) through this side's endpoints. If gas cannot be estimated the
/// transaction would revert, so it is refused by name and not sent.
pub(super) fn estimate_on(a: GasAsk) -> Result<crate::task::Done, crate::fault::Fault> {
    use zikaron_anchor::send::NoGas;
    let GasAsk { count, chain, call, data, eps } = a;
    // A failed estimate is reported as "will revert" only when the node itself refused the call, judged by
    // the same table the command line uses (`chainx::ask_call`, `said::refuses_the_call`). Unreachable nodes,
    // timeouts, rate limiting and other errors keep their own codes, so a person whose node is down is not
    // told the transaction will fail. The estimate is made at the lowest head every endpoint has reached
    // (`head_block`); failing to get the head is never about the call.
    let n = zikaron_anchor::send::estimate_gas(
        || crate::chainx::head_block(&eps, chain).map(|(height, _)| height).map_err(|f| (f, false)),
        |params| crate::chainx::ask_call(&eps, "eth_estimateGas", params).map(|r| r.value),
        call,
        |(_, about_the_call): &(crate::fault::Fault, bool)| !about_the_call,
    )
    .map_err(|no| match no {
        NoGas::Network((f, _)) => f,
        NoGas::Refused((f, _)) => crate::fault::Fault::known(crate::fault::Known::GasRefused, f.evidence()),
        NoGas::NotText(other) => crate::fault::Fault::known(
            crate::fault::Known::ChainShape,
            crate::lang::filln(crate::lang::Key::Tail033, &[&format!("{:?}", other)]),
        ),
        NoGas::Unreadable(hex) => crate::fault::Fault::known(crate::fault::Known::ChainShape, crate::lang::filln(crate::lang::Key::Tail034, &[&(hex).to_string()])),
        // No transaction carries more than `send::GAS_LIMIT`: an estimate above it would run out of gas on
        // chain with the fee still paid, so it is refused with the node's own number before anything is
        // shown or sent.
        NoGas::OverCap(n) => crate::fault::Fault::known(crate::fault::Known::GasRefused, format!("{n} > {}", zikaron_anchor::send::GAS_LIMIT)),
    })?;
    let gas = n;
    // The node was reached, so fetch the chain's current time too (failure does not block the estimate).
    let head_time = crate::chainx::head_time(&eps, chain).ok().map(|(time, _, _)| time);
    // The fee cap comes from the chain's base fee and the gas limit from this estimate
    // (`Fees::with_estimate`), so the confirmation card, the pre-send balance gate and the signed transaction
    // all use the same values.
    let (fees, fees_left) = crate::chainx::fees(&eps, chain);
    let fees = fees.with_estimate(gas);
    // Also return the call data used for estimating: the `input` of the sent transaction, read back from the
    // chain, must match it byte for byte.
    Ok(crate::task::Done::Gas { count, gas, calldata: zikaron::hexfmt::encode(&data), fees, fees_left, head_time })
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
    // only inside `sign::anchor_send` for exactly that send, and zeroed when dropped. The fee reading comes
    // with the estimate; tests that set gas directly without estimating use `Fees::fallback`.
    let fees = shell.fees.unwrap_or_else(zikaron_anchor::send::Fees::fallback);
    // What the exit gate needs is taken here; the chain is read in the task as its last step before sending.
    let ask = crate::exitgate::ask_of(shell)?;
    Ok(shell.tasks.spawn(Kind::Anchor, move || anchor_batch(b, secret, root, gas, fees, ask)))
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
    matches!(
        shell.tasks.spawn(Kind::Anchor, move || confirm_batch(root, urls, chain, txs, ids, None, wait, &backoff, sender, nonce)),
        Spawned::Started
    )
}

/// The account a batch is sent from and the registry it calls, read on the UI thread. Used to judge a batch no
/// node holds (via the account's on-chain nonce) and to resend it (`confirm_batch`). `None` if either is
/// missing.
fn sender_of(shell: &Shell) -> Option<Sender> {
    Some(Sender { from: shell.anchor?.0, registry: shell.settings.registry?.0 })
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
    let _ = shell.tasks.spawn(Kind::Anchor, move || confirm_batch(root, urls, chain, txs, ids, gas, wait, &backoff, sender, nonce));
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

/// Pre-send balance gate. The required amount comes from the same source as the transaction (`Fees::cap_wei`:
/// fee cap times gas limit, exactly what the node checks at broadcast). The balance is read now by the single
/// balance reader (`chainx::balance`): only this chain's nodes, at one pinned block (the lowest head all have
/// reached, so a lagging node never causes a split), with differing answers refused by name and a lone answer
/// marked single-source. An unreadable balance is refused for that reason, never assumed sufficient. The
/// chain's own refusal at broadcast remains as a backstop. The evidence tail is `need=<wei> have=<wei>`: tests
/// read it, and the UI shows both amounts in ETH.
pub(super) fn funds_gate(b: &Batch, secret: &crate::key::Secret, fees: zikaron_anchor::send::Fees) -> Result<(), crate::fault::Fault> {
    let who = secret.address().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::KeyMalformed, crate::lang::t(crate::lang::Key::Tail013).to_string())
    })?;
    // The balance uses every endpoint for this chain: those that answer and agree count; those that do not
    // answer are skipped and reported.
    let eps: Vec<crate::chainx::Endpoint> = b.urls.iter().map(|u| crate::chainx::Endpoint::at(b.chain, u.clone())).collect();
    let (have, _) = crate::chainx::balance(&eps, b.chain, &who)?;
    funds_check(fees.cap_wei(), have)
}

/// The balance gate's comparison: a balance at least equal to the maximum spend passes (exactly enough
/// included); one wei less is refused by name with both numbers (`funds_tail`).
pub fn funds_check(need: u128, have: u128) -> Result<(), crate::fault::Fault> {
    if have < need {
        return Err(crate::fault::Fault::known(crate::fault::Known::InsufficientFunds, funds_tail(need, have)));
    }
    Ok(())
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

/// Hands this batch to the anchoring crate. This is the only place on the anchoring path that touches the
/// chain.
///
/// It is separate from `send_batch` so each precondition has its own place: `send_batch`'s refusals (no chain
/// id, no registry, no endpoint, empty queue) come first, and the hand-off itself has its own failure point.
///
/// It goes only as far as "submitted": when the echo matches, the entries are saved as submitted (with
/// transaction hash and chain id), `Done::Submitted` is returned, and the shell starts the receipt wait
/// (`confirm_batch`). A refusal before broadcast is recorded as not sent.
pub(super) fn anchor_batch(
    b: Batch,
    secret: crate::key::Secret,
    root: std::path::PathBuf,
    gas: Option<u64>,
    fees: zikaron_anchor::send::Fees,
    ask: crate::exitgate::Ask,
) -> Result<Done, crate::fault::Fault> {
    let home = crate::home::Home::open(&root)?;
    let mut b = b;
    let refused = |f: crate::fault::Fault| -> crate::fault::Fault {
        let said = f.which().map(|k| k.as_str().to_string()).unwrap_or_default();
        let _ = crate::queue::amend(&home, |q| q.mark(&b.ids, crate::queue::Step::Refused { said }));
        f
    };
    // Pre-send balance gate: if the balance does not cover fee cap times gas, refuse by name as
    // `INSUFFICIENT_FUNDS` with both amounts and broadcast nothing. It runs after the show-before-send gate
    // (`gas_shown` in `send_batch`), not instead of it.
    b.urls = serving_urls(&b.urls, b.chain).map_err(refused)?;
    funds_gate(&b, &secret, fees).map_err(refused)?;
    // The exit gate, last before the chain: every anchor of this ledger's lineage, read now, must be held here.
    let pass = crate::exitgate::pass(&ask).map_err(refused)?;
    // The code that touches the chain (sign, broadcast, check the echo) lives in `sign::anchor_send`, the only
    // file where the key is lent out.
    let sent = crate::sign::anchor_send(&pass, &secret, &b.urls, b.chain, Some(b.registry.0), &b.hashes, fees, &b.backoff).map_err(refused)?;
    let tx = zikaron::hexfmt::encode(&sent.hash);
    // Once the echo matches, save "submitted" so that after a restart this transaction is awaited, not resent.
    // If the process dies before the echo, the batch stays queued and is sent again on request (a hash saved
    // before the echo that no node holds would leave the batch awaited forever, since no wait can tell a
    // dropped transaction from a lagging node).
    let (_, queue) = crate::queue::amend(&home, |q| q.mark(&b.ids, crate::queue::Step::Submitted { tx: tx.clone(), chain: b.chain, nonce: sent.nonce }))?;
    // The receipt wait asks the endpoint that accepted it first.
    Ok(Done::Submitted { tx, chain: b.chain, url: sent.taken, ids: b.ids, gas, queue })
}

/// The batch's nodes that serve its chain (each checked once per process, `chainx::serving`); the pre-send
/// balance, nonce and broadcast use only these. Nodes serving another chain are left out and reported as a
/// `WRONG_CHAIN` refusal naming them. Runs on the task's thread.
fn serving_urls(urls: &[crate::chainx::NodeAddr], chain: u64) -> Result<Vec<crate::chainx::NodeAddr>, crate::fault::Fault> {
    let eps: Vec<crate::chainx::Endpoint> = urls.iter().map(|u| crate::chainx::Endpoint::at(chain, u.clone())).collect();
    let (kept, _) = crate::chainx::serving(&eps, chain)?;
    Ok(kept.into_iter().map(|e| e.url).collect())
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
    let b = Batch { hashes: crate::queue::Queue::hashes(&ids)?, ids, chain: stuck.chain, registry, urls, backoff: shell.send_backoff.clone() };
    let secret = signing_key(shell, crate::sign::Use::Anchor)?;
    let root = shell.home.as_ref().map(|h| h.root().to_path_buf()).ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    let ask = crate::exitgate::ask_of(shell)?;
    Ok(shell.tasks.spawn(Kind::Anchor, move || resend_batch(b, stuck, fees, secret, root, ask)))
}

/// The resend task: check the balance for the new cap, pass the exit gate, sign, then record the new hash with
/// the batch (`Resent`) BEFORE the single broadcast, so any resend that goes out is one the queue watches (a
/// write failing after a broadcast would leave the batch watching only transactions that can no longer be
/// included). If a node refuses the broadcast the hash is removed again; if the bytes may be out it stays; a
/// refusal before broadcast changes nothing. Transactions already out stay awaited either way (never marked
/// refused, since the first may still be included), and a recorded hash no node holds is skipped by the wait.
fn resend_batch(b: Batch, stuck: crate::task::Stuck, fees: zikaron_anchor::send::Fees, secret: crate::key::Secret, root: std::path::PathBuf, ask: crate::exitgate::Ask) -> Result<Done, crate::fault::Fault> {
    let home = crate::home::Home::open(&root)?;
    let mut b = b;
    b.urls = serving_urls(&b.urls, b.chain)?;
    funds_gate(&b, &secret, fees)?;
    let pass = crate::exitgate::pass(&ask)?;
    let (raw, hash) = crate::sign::bump_sign(&pass, &secret, &b.urls, &stuck, fees)?;
    let tx = zikaron::hexfmt::encode(&hash);
    let before = if stuck.txs.len() == 1 {
        crate::queue::Step::Submitted { tx: stuck.txs[0].clone(), chain: b.chain, nonce: Some(stuck.nonce) }
    } else {
        crate::queue::Step::Resent { txs: stuck.txs.clone(), chain: b.chain, nonce: Some(stuck.nonce) }
    };
    // Signing the same bytes again (a batch no node holds, offered at its original price) gives the same
    // transaction: broadcast it again but watch it once.
    let mut txs = stuck.txs.clone();
    if !txs.contains(&tx) {
        txs.push(tx.clone());
    }
    let after = match txs.as_slice() {
        [one] => crate::queue::Step::Submitted { tx: one.clone(), chain: b.chain, nonce: Some(stuck.nonce) },
        _ => crate::queue::Step::Resent { txs, chain: b.chain, nonce: Some(stuck.nonce) },
    };
    let (_, queue) = crate::queue::amend(&home, |q| q.mark(&b.ids, after))?;
    let sent = match crate::sign::bump_broadcast(&pass, &b.urls, b.chain, &raw, hash, &b.backoff) {
        Ok(s) => s,
        Err(f) => {
            // A node's own verdict (underpriced, nonce used, …) means the bytes were not accepted, so the hash
            // is removed. Any other failure (no node answered, a network error after the bytes were sent) may
            // leave them in a mempool: the hash stays watched, and the wait skips it while no node holds it.
            if crate::chainx::next_after(&f) == crate::chainx::Next::Stop {
                let _ = crate::queue::amend(&home, |q| q.mark(&b.ids, before));
            }
            return Err(f);
        }
    };
    Ok(Done::Submitted { tx, chain: b.chain, url: sent.taken, ids: b.ids, gas: None, queue })
}

/// Waits at most `wait` for the receipt, then returns; if not yet included the batch stays submitted (or
/// resent) and the next call keeps waiting.
///
/// Every transaction of the batch is awaited (`txs`, oldest first: one, or more after fee-bump resends),
/// sharing the wait. They share one nonce, so at most one can be included, and the first found included
/// counts. If none is included by the end, the latest is read as the chain holds it, with the current price
/// ([`stuck_of`]): this is what the "resend with higher fees" button offers. If none is included and no node
/// holds any (dropped from every mempool, or the nonce used by another transaction), the batch is judged by the
/// sending account's on-chain nonce against its own ([`unheld`]): voided and requeued (reported by name), or
/// offered again as recorded at its nonce and the current price. Whether a node still holds the transaction
/// never decides whether action is possible: a batch no node holds is exactly the one that needs it.
#[allow(clippy::too_many_arguments)]
pub(super) fn confirm_batch(
    root: std::path::PathBuf,
    urls: Vec<crate::chainx::NodeAddr>,
    chain: u64,
    txs: Vec<String>,
    ids: Vec<String>,
    gas: Option<u64>,
    wait: std::time::Duration,
    backoff: &[std::time::Duration],
    sender: Option<Sender>,
    nonce: Option<u64>,
) -> Result<Done, crate::fault::Fault> {
    let url = urls.first().map(|u| u.for_transport().to_string()).unwrap_or_default();
    let mut hashes: Vec<[u8; 32]> = Vec::with_capacity(txs.len());
    for tx in &txs {
        let h: [u8; 32] = zikaron::hexfmt::decode(tx)
            .and_then(|b| b.try_into().ok())
            .ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::ContentShape, tx.clone()))?;
        hashes.push(h);
    }
    if hashes.is_empty() {
        return Err(crate::fault::Fault::known(crate::fault::Known::QueueEmpty, ids.join(" ")));
    }
    // Ask each endpoint for the receipt, starting with the one that accepted the transaction; if none can be
    // opened, refuse by name.
    let mut opened: Vec<Box<dyn zikaron_anchor::rpc::Endpoint + Send>> = urls.iter().filter_map(|u| crate::chainx::endpoint_at(u.for_transport())).collect();
    if opened.is_empty() {
        return Err(crate::fault::Fault::known(crate::fault::Known::SettingsShape, crate::chainx::address_said(&url)));
    }
    let mut refs: Vec<&mut dyn zikaron_anchor::rpc::Endpoint> = opened.iter_mut().map(|b| b.as_mut() as &mut dyn zikaron_anchor::rpc::Endpoint).collect();
    // One wait shared between the transactions; the first included counts. Otherwise report the most
    // informative result: some nodes with a receipt and some without, then "not yet", then unreachable.
    let share = wait / hashes.len() as u32;
    let rank = |c: &zikaron_anchor::send::Confirm| match c {
        zikaron_anchor::send::Confirm::Included { .. } => 3,
        zikaron_anchor::send::Confirm::Split { .. } => 2,
        zikaron_anchor::send::Confirm::NotYet => 1,
        zikaron_anchor::send::Confirm::Unreachable(_) => 0,
    };
    let mut confirm: Option<zikaron_anchor::send::Confirm> = None;
    let mut tx = txs.last().cloned().unwrap_or_default();
    for (h, t) in hashes.iter().zip(&txs) {
        let c = zikaron_anchor::send::confirm_each(&mut refs, h, share, backoff);
        let included = matches!(c, zikaron_anchor::send::Confirm::Included { .. });
        if confirm.as_ref().is_none_or(|was| rank(&c) > rank(was)) {
            confirm = Some(c);
            if included {
                tx = t.clone();
                break;
            }
        }
    }
    let confirm = confirm.unwrap_or(zikaron_anchor::send::Confirm::NotYet);
    // What the sent transaction actually contains: `input` read back from the chain by hash, never rebuilt
    // here. If unreadable it is empty (the UI says "not read"); the broadcast already happened, so this must
    // not fail the call.
    let calldata = crate::chainx::tx_as_held(&urls, chain, &tx).as_ref().and_then(|v| text_member(v, "input")).unwrap_or_default();
    let state = match &confirm {
        zikaron_anchor::send::Confirm::Included { status, block_number } => receipt_state(*status, *block_number),
        zikaron_anchor::send::Confirm::NotYet => "not yet in a block".to_string(),
        zikaron_anchor::send::Confirm::Unreachable(e) => unheard_state(&e.to_string()),
        // Some nodes have the receipt and others not yet, even after re-asking: not treated as anchored on the
        // word of some nodes; it stays submitted and the next call asks again.
        zikaron_anchor::send::Confirm::Split { has, not_yet } => format!("receipt at {} only; not yet at {}", has.join(" "), not_yet.join(" ")),
    };
    let anchored = matches!(confirm, zikaron_anchor::send::Confirm::Included { status: 1, .. });
    // Entries leave the queue only when the receipt reports success (a status other than 1 is not an anchor);
    // a failed send stays queued for retry. The included transaction's block number is recorded too.
    // If not included by the end of this wait: the latest transaction of the batch a node still holds (later
    // ones recorded but never accepted, or dropped from mempools, are skipped).
    let held = match &confirm {
        zikaron_anchor::send::Confirm::NotYet => Some(txs.iter().rev().find_map(|t| crate::chainx::tx_as_held(&urls, chain, t))),
        _ => None,
    };
    // Held by no node: judge it by the account's on-chain nonce (`unheld`) rather than waiting silently.
    let unheld = match (&confirm, &held, sender) {
        (zikaron_anchor::send::Confirm::NotYet, Some(None), Some(who)) => unheld(&urls, chain, &txs, &ids, nonce, who, &mut refs),
        _ => Unheld::Wait,
    };
    let state = match &unheld {
        Unheld::Void => "void: held by no node, its nonce used by another transaction".to_string(),
        Unheld::Again(_) => "held by no node; its nonce unused".to_string(),
        Unheld::Wait => state,
    };
    let voided = matches!(unheld, Unheld::Void);
    let home = crate::home::Home::open(&root)?;
    let (dropped, queue) = match (&confirm, &held) {
        (zikaron_anchor::send::Confirm::Included { status: 1, block_number }, _) => {
            let n = *block_number;
            // Removal goes only through `settle`, which saves the inclusion in the same write.
            crate::queue::settle(&home, &ids, true, Some(&crate::queue::Inclusion { tx: tx.clone(), chain, block: n }))?
        }
        (zikaron_anchor::send::Confirm::Included { .. }, _) => {
            crate::queue::amend(&home, |q| {
                q.mark(&ids, crate::queue::Step::Reverted { tx: tx.clone(), chain });
                0
            })?
        }
        // Void: requeue every entry of the batch (`Queued`, sendable again; nothing else changes).
        _ if voided => crate::queue::amend(&home, |q| {
            q.mark(&ids, crate::queue::Step::Queued);
            0
        })?,
        // Not yet included: still in flight; the next call keeps waiting (no resend).
        _ => crate::queue::settle(&home, &ids, false, None)?,
    };
    // What can be done now, from the transaction a node holds and the current price; if no node holds it and
    // its nonce is unused, the batch as recorded at the current price.
    let stuck = match unheld {
        Unheld::Again(s) => Some(s),
        _ => held.flatten().and_then(|v| stuck_of(&urls, chain, &txs, &v)),
    };
    Ok(Done::Anchored {
        tx,
        chain,
        confirmed: anchored,
        state,
        sent: ids.len(),
        dropped,
        queue: queue.items,
        gas,
        calldata,
        stuck,
        voided,
    })
}

/// The account a batch is sent from and the registry it calls (`sender_of`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Sender {
    pub from: [u8; 20],
    pub registry: [u8; 20],
}

/// What becomes of a batch no node holds ([`unheld`]).
enum Unheld {
    /// Its nonce is used on chain and none of its transactions was included: void, back in the queue.
    Void,
    /// Its nonce is unused: the batch as recorded may be sent again (the offer).
    Again(crate::task::Stuck),
    /// Not decided (on-chain nonce unread, or a receipt found on re-asking): keep waiting.
    Wait,
}

/// Judges a batch whose transactions no node holds by comparing the sending account's on-chain nonce with the
/// batch's own (`nonce`, recorded at signing; queue files from older versions lack it, so the account's next
/// nonce stands in, and such a batch is offered again rather than voided on a guess):
/// - **used** (every answering node's nonce is past it): each transaction's receipt is asked once more (a node
///   past the nonce has the block, and the receipt if it was this batch's); if all are "not yet" the batch is
///   void, otherwise it waits.
/// - **unused**: the batch as recorded here (its entries, the registry call over them, at its nonce) is offered
///   again at the current price ([`again_of`]).
///
/// The rule is the anchoring crate's (`zikaron_anchor::send::unheld`, also used by the command line's `anchor`
/// on a rerun); this side supplies the queries (every node serving the chain).
fn unheld(urls: &[crate::chainx::NodeAddr], chain: u64, txs: &[String], ids: &[String], nonce: Option<u64>, who: Sender, refs: &mut [&mut dyn zikaron_anchor::rpc::Endpoint]) -> Unheld {
    let on_chain = chain_nonce(urls, chain, &who.from);
    let mine = nonce.or(on_chain).unwrap_or_default();
    let none_included = || {
        txs.iter().all(|t| {
            let h: Option<[u8; 32]> = zikaron::hexfmt::decode(t).and_then(|b| b.try_into().ok());
            h.is_some_and(|h| matches!(zikaron_anchor::send::confirm_each(refs, &h, std::time::Duration::ZERO, &[]), zikaron_anchor::send::Confirm::NotYet))
        })
    };
    match zikaron_anchor::send::unheld(on_chain, mine, none_included) {
        zikaron_anchor::send::Unheld::Void => Unheld::Void,
        zikaron_anchor::send::Unheld::Wait => Unheld::Wait,
        zikaron_anchor::send::Unheld::Unused => match again_of(urls, chain, txs, ids, mine, who) {
            Some(s) => Unheld::Again(s),
            None => Unheld::Wait,
        },
    }
}

/// The sending account's nonce at the `latest` block, the lowest among nodes serving this chain (so a lagging
/// node never makes a batch look void). `None` when no node answers.
fn chain_nonce(urls: &[crate::chainx::NodeAddr], chain: u64, from: &[u8; 20]) -> Option<u64> {
    let eps: Vec<crate::chainx::Endpoint> = urls.iter().map(|u| crate::chainx::Endpoint::at(chain, u.clone())).collect();
    let (eps, _) = crate::chainx::serving(&eps, chain).ok()?;
    let nodes: Vec<(String, Box<dyn zikaron_anchor::rpc::Endpoint + Send>)> = eps.iter().filter_map(|e| crate::chainx::endpoint_at(e.url.for_transport()).map(|h| (e.url.for_transport().to_string(), h))).collect();
    let params = zikaron::json::Value::Arr(vec![zikaron::json::Value::Str(zikaron::hexfmt::encode(from)), zikaron::json::Value::Str("latest".into())]);
    zikaron_anchor::endpoints::ask_each(nodes, "eth_getTransactionCount", &params)
        .into_iter()
        .filter_map(|(_, got)| match got.ok().and_then(|w| zikaron_anchor::wire::to_core(&w)) {
            Some(zikaron::json::Value::Str(x)) => crate::chainx::wei(&x).and_then(|n| u64::try_from(n).ok()),
            _ => None,
        })
        .min()
}

/// Offers a batch no node holds again, as recorded here: the registry call over its entries, to the registry,
/// from the account, at `nonce`, with the current price and a fresh gas estimate (nothing is replaced, so no
/// bump over the last). Once resent [`crate::queue::RESENDS_MAX`] times nothing more is offered; if the price or
/// gas cannot be read, nothing is offered and the wait goes on.
fn again_of(urls: &[crate::chainx::NodeAddr], chain: u64, txs: &[String], ids: &[String], nonce: u64, who: Sender) -> Option<crate::task::Stuck> {
    let input = zikaron_anchor::send::registry_calldata(&crate::queue::Queue::hashes(ids).ok()?);
    let eps: Vec<crate::chainx::Endpoint> = urls.iter().map(|u| crate::chainx::Endpoint::at(chain, u.clone())).collect();
    let now = crate::chainx::fee_reading(&eps, chain);
    let gas = zikaron_anchor::send::estimate_gas(
        || crate::chainx::head_block(&eps, chain).map(|(height, _)| height).map_err(|f| (f, false)),
        |params| crate::chainx::ask_call(&eps, "eth_estimateGas", params).map(|r| r.value),
        zikaron_anchor::send::estimate_call(&who.from, &who.registry, &input),
        |(_, about_the_call): &(crate::fault::Fault, bool)| !about_the_call,
    )
    .ok();
    let offer = match (txs.len() > crate::queue::RESENDS_MAX, now.base, gas) {
        (true, _, _) => crate::task::Offer::Spent,
        (false, Some(base_now), Some(gas)) => crate::task::Offer::Unheld { fees: now.fees.with_estimate(gas), base_now },
        (false, _, _) => crate::task::Offer::PriceUnread,
    };
    let sent = match &offer {
        crate::task::Offer::Unheld { fees, .. } => *fees,
        _ => now.fees,
    };
    Some(crate::task::Stuck { txs: txs.to_vec(), chain, from: who.from, nonce, to: who.registry, input, sent, offer, left: now.left })
}


fn text_member(v: &zikaron::json::Value, k: &str) -> Option<String> {
    match v {
        zikaron::json::Value::Obj(m) => m.iter().find(|(n, _)| n == k).and_then(|(_, v)| match v {
            zikaron::json::Value::Str(x) => Some(x.clone()),
            _ => None,
        }),
        _ => None,
    }
}

/// A batch not included by the end of a wait: its last transaction as the chain holds it (`held`) and what can
/// be done now. `None` if the transaction's fields cannot be read (nothing is offered then).
fn stuck_of(urls: &[crate::chainx::NodeAddr], chain: u64, txs: &[String], held: &zikaron::json::Value) -> Option<crate::task::Stuck> {
    let qty = |k: &str| text_member(held, k).as_deref().and_then(crate::chainx::wei).and_then(|n| u64::try_from(n).ok());
    let addr = |k: &str| text_member(held, k).as_deref().and_then(zikaron::hexfmt::decode).and_then(|b| <[u8; 20]>::try_from(b).ok());
    let input = text_member(held, "input").as_deref().and_then(zikaron::hexfmt::decode)?;
    let sent = zikaron_anchor::send::Fees { max_fee: qty("maxFeePerGas")?, priority: qty("maxPriorityFeePerGas")?, gas_limit: qty("gas")?, from_chain: true };
    let (from, nonce, to) = (addr("from")?, qty("nonce")?, addr("to")?);
    let (offer, left) = if txs.len() > crate::queue::RESENDS_MAX {
        (crate::task::Offer::Spent, Vec::new())
    } else {
        let eps: Vec<crate::chainx::Endpoint> = urls.iter().map(|u| crate::chainx::Endpoint::at(chain, u.clone())).collect();
        let now = crate::chainx::fee_reading(&eps, chain);
        let offer = match now.base {
            Some(base_now) if base_now > sent.max_fee => crate::task::Offer::Resend { fees: zikaron_anchor::send::Fees::replacing(sent, now.fees), base_now },
            Some(base_now) => crate::task::Offer::NotAbove { base_now },
            None => crate::task::Offer::PriceUnread,
        };
        (offer, now.left)
    };
    Some(crate::task::Stuck { txs: txs.to_vec(), chain, from, nonce, to, input, sent, offer, left })
}
