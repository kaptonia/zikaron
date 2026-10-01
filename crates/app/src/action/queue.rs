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
    let urls: Vec<String> = shell.endpoints.iter().filter(|e| e.chain == chain).map(|e| e.url.clone()).collect();
    if urls.is_empty() {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::NoEndpoint,
            crate::lang::filln(crate::lang::Key::Tail031, &[&(chain).to_string()]),
        ));
    }
    Ok(Batch { ids, hashes, chain, registry, urls, backoff: shell.send_backoff.clone() })
}

/// Estimate gas once. The call data is byte-identical to the real transaction (both assembled by the
/// anchoring crate's `registry_calldata`), so the estimate and the send are the same transaction; if it
/// cannot be estimated, the transaction would revert, so it is refused by name at once and not sent.
pub(super) fn estimate_gas(shell: &mut Shell, count: usize) -> Result<(u64, String, zikaron_anchor::send::Fees), crate::fault::Fault> {
    let b = batch(shell, count)?;
    let who = shell.anchor.ok_or_else(|| {
        crate::fault::Fault::known(
            crate::fault::Known::KeychainMissing,
            crate::lang::t(crate::lang::Key::Tail032).to_string(),
        )
    })?;
    let data = zikaron_anchor::send::registry_calldata(&b.hashes);
    let params = zikaron::json::Value::Arr(vec![zikaron::json::Value::Obj(vec![
        ("data".into(), zikaron::json::Value::Str(zikaron::hexfmt::encode(&data))),
        ("from".into(), zikaron::json::Value::Str(who.hex())),
        ("to".into(), zikaron::json::Value::Str(b.registry.hex())),
    ])]);
    let eps: Vec<crate::chainx::Endpoint> = shell
        .endpoints
        .iter()
        .filter(|e| e.chain == b.chain)
        .cloned()
        .collect();
    // A failed estimate speaks only of the form where the node answered and refused: unreachable, timeout and
    // rate limiting are handed out with the codes the chain-read exit dispatches, never said as "this
    // transaction will revert" (a person whose node is down would otherwise be told the transaction will
    // fail).
    let r = crate::chainx::ask(&eps, "eth_estimateGas", &params).map_err(|f| {
        if crate::watchx::is_network(&f) {
            f
        } else {
            crate::fault::Fault::known(crate::fault::Known::GasRefused, f.evidence())
        }
    })?;
    let hex = match &r.value {
        zikaron::json::Value::Str(s) => s.clone(),
        other => {
            return Err(crate::fault::Fault::known(
                crate::fault::Known::ChainShape,
                crate::lang::filln(crate::lang::Key::Tail033, &[&format!("{:?}", other)]),
            ))
        }
    };
    let n = crate::chainx::wei(&hex).ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::ChainShape, crate::lang::filln(crate::lang::Key::Tail034, &[&(hex).to_string()]))
    })?;
    // Estimating talked to the node: record the chain's current time as well (failing to get it does not
    // block the estimate).
    if let Ok((time, _, _)) = crate::chainx::head_time(&eps, b.chain) {
        shell.note_chain_time(time);
    }
    // The fee cap is computed from the chain's base fee: the confirmation card's cell and the pre-send
    // balance gate read the same value.
    let fees = crate::chainx::fees(&eps, b.chain);
    // Also hand out the call data given to the node for estimating: the `input` of the sent transaction read
    // back from the chain must match it byte for byte.
    Ok((n as u64, zikaron::hexfmt::encode(&data), fees))
}

/// The "show before send" question: whether this batch's gas was estimated, and for this count.
///
/// A separate function: the only entry to sending comes after it, and if it cannot answer there is no next
/// statement.
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
    // Show before send. If this batch's gas was not estimated, or was estimated for another count, it does
    // not pass.
    let gas = Some(gas_shown(shell, count)?);
    // Anchoring does not ask "can this ledger still be written". Putting hashes of already recorded bytes on
    // chain (law §9) is not appending entries, and the succession entry itself must be anchorable, or the new
    // key's side can never verify it and the handover stalls halfway. So this lets the succession refusal
    // through and still blocks the others (lock, broken chain, pen). This closes the "entries queued before
    // the handover can never reach the chain" form; what is loosened is exactly anchoring, which adds not one
    // byte to the ledger.
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
    // The private key bytes never enter this layer. The `Secret` is moved whole into the background pass; the
    // loan happens in `sign::anchor_send`, covering exactly that send, and the bytes are zeroed when it goes
    // out of scope. This layer cannot get the thirty-two bytes. The fee reading comes with the estimate; test
    // steps that set gas directly without estimating take the fallback (`Fees::fallback`).
    let fees = shell.fees.unwrap_or_else(zikaron_anchor::send::Fees::fallback);
    // What the exit gate needs, taken here; the chain is read in the pass, as its last step before sending.
    let ask = crate::exitgate::ask_of(shell)?;
    Ok(shell.tasks.spawn(Kind::Anchor, move || anchor_batch(b, secret, root, gas, fees, ask)))
}

/// Resume waiting for "submitted" transactions: entries the queue file records as submitted without a receipt
/// get a pass that only waits for the receipt, without resending. Opening the home and the window's periodic
/// check each ask it; not started when one of the same kind is in flight. Returns whether a pass started.
pub fn resume(shell: &mut Shell) -> bool {
    // This mark only says "cannot resume now": cleared at the start of every pass, never carrying the
    // previous pass's words (a changed home, a settled batch or a node added back should all make the face
    // line disappear at once).
    shell.resume_blocked = None;
    if shell.tasks.in_flight(Kind::Anchor) || !shell.writable() {
        return false;
    }
    let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) else { return false };
    let Some((tx, chain, ids)) = shell.queue.submitted().into_iter().next() else { return false };
    let urls = receipt_urls(shell, chain, None);
    if urls.is_empty() {
        // Failing to resume must be said: a submitted batch is waiting for a receipt, and the current
        // settings have no node for that chain (basis changed, node removed). Returning false silently would
        // retry every fifteen seconds, those entries would spin on "waiting for receipt" forever and never
        // join the next batch, with not a word on the face. It is recorded on the shell, and the queue page
        // says so.
        shell.resume_blocked = Some(chain);
        return false;
    }
    let wait = shell.anchor_wait;
    matches!(
        shell.tasks.spawn(Kind::Anchor, move || confirm_batch(root, urls, chain, tx, ids, None, wait)),
        Spawned::Started
    )
}

/// Right after unlocking, once: what the locked time missed. The self-audit (it raises the missed notices,
/// once each), the vault review with its sentinel, and the receipt wait of a submitted batch; each starts only
/// where it can (a home with a basis and nodes), and nothing is said for what cannot start.
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

/// Right after a batch is confirmed on chain: one self-audit now, the existing one (its label moves the pen,
/// as always). Started where it can start (a basis, nodes); what cannot start says so through the audit's own
/// refusal on the face.
pub fn audit_after_send(shell: &mut Shell) {
    if let Err(f) = start_audit(shell) {
        shell.faults.push(f);
    }
}

/// After a broadcast lands, wait for the receipt (called where the shell receives messages).
pub fn wait_submitted(shell: &mut Shell, tx: String, chain: u64, url: String, ids: Vec<String>, gas: Option<u64>) {
    let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) else { return };
    let urls = receipt_urls(shell, chain, Some(&url));
    let wait = shell.anchor_wait;
    let _ = shell.tasks.spawn(Kind::Anchor, move || confirm_batch(root, urls, chain, tx, ids, gas, wait));
}

/// Which places to ask for the receipt: the place that took the transaction first, then this chain's other
/// configured endpoints (deduplicated).
pub(super) fn receipt_urls(shell: &Shell, chain: u64, taken: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = taken.map(|u| vec![u.to_string()]).unwrap_or_default();
    for e in shell.endpoints.iter().filter(|e| e.chain == chain) {
        if !out.iter().any(|u| *u == e.url) {
            out.push(e.url.clone());
        }
    }
    out
}

/// Pre-send balance gate. The required amount has the same source as the anchoring transaction
/// (`Fees::cap_wei`: fee cap times gas limit, exactly what the node checks at broadcast). The balance is read
/// now from the anchoring endpoints; unreadable is refused by that named reason, never guessing "probably
/// enough" (as with show before send: no answer, no pass). The evidence tail is `need=<wei> have=<wei>`:
/// tests read it, and the face states the two numbers in ETH.
pub(super) fn funds_gate(b: &Batch, secret: &crate::key::Secret, fees: zikaron_anchor::send::Fees) -> Result<(), crate::fault::Fault> {
    let who = secret.address().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::KeyMalformed, crate::lang::t(crate::lang::Key::Tail013).to_string())
    })?;
    // The balance uses the whole endpoint table (endpoint rule: places that answer and agree count; those
    // that do not answer are skipped, and it says so).
    let eps: Vec<crate::chainx::Endpoint> = b.urls.iter().map(|u| crate::chainx::Endpoint { chain: b.chain, url: u.clone() }).collect();
    let have = crate::chainx::balance_first(&eps, &who)?;
    let need = fees.cap_wei();
    if have < need {
        return Err(crate::fault::Fault::known(crate::fault::Known::InsufficientFunds, funds_tail(need, have)));
    }
    Ok(())
}

/// How the receipt sentence is written (one name, one home: `Done::Anchored.state` is written and read only
/// here).
pub fn receipt_state(status: u64, block: u64) -> String {
    format!("status {status} · block {block}")
}

/// Receipt status is not 1 (included but execution failed; law §9.1: neither form is an anchor). The plain
/// words of `SEND_FAILED` are kept for this form only.
pub fn receipt_failed(state: &str) -> bool {
    state
        .strip_prefix("status ")
        .and_then(|x| x.split_whitespace().next())
        .map(|x| x != "1")
        .unwrap_or(false)
}

/// How the balance gate's evidence tail is written (one name, one home: written and read only here).
pub fn funds_tail(need: u128, have: u128) -> String {
    format!("need={need} have={have}")
}

/// Read required and current amounts back from the evidence tail (`None` when unreadable: that is the
/// node-refusal form, whose words lack the two numbers).
pub fn funds_of(tail: &str) -> Option<(u128, u128)> {
    let need = tail.split_whitespace().find_map(|x| x.strip_prefix("need="))?.parse().ok()?;
    let have = tail.split_whitespace().find_map(|x| x.strip_prefix("have="))?.parse().ok()?;
    Some((need, have))
}

/// Hand this batch to the base layer to anchor. One name, one home: the statement on the anchoring path that
/// actually touches the chain lives only here.
///
/// It lives apart from `send_batch` so each precondition has its own place: the four refusals in `send_batch`
/// (no chain id, no registry, no endpoint, empty queue) are answered before this statement, and "this batch
/// really went to the anchoring crate" has a failure point of its own.
///
/// This pass goes only as far as "submitted": when the echo matches, the entries are recorded as submitted
/// (with transaction hash and chain id) and saved, `Done::Submitted` is returned, and the shell starts the
/// receipt wait (`confirm_batch`). Refused before broadcast is recorded as "not sent".
pub(super) fn anchor_batch(
    b: Batch,
    secret: crate::key::Secret,
    root: std::path::PathBuf,
    gas: Option<u64>,
    fees: zikaron_anchor::send::Fees,
    ask: crate::exitgate::Ask,
) -> Result<Done, crate::fault::Fault> {
    let home = crate::home::Home::open(&root)?;
    let refused = |f: crate::fault::Fault| -> crate::fault::Fault {
        let said = f.which().map(|k| k.as_str().to_string()).unwrap_or_default();
        let _ = crate::queue::amend(&home, |q| q.mark(&b.ids, crate::queue::Step::Refused { said }));
        f
    };
    // Pre-send balance gate: whether the balance covers fee cap times gas is asked once before broadcast; if
    // not, refused by name as `INSUFFICIENT_FUNDS` with required and current amounts, and nothing is
    // broadcast. It sits after the show-before-send gate (`send_batch`'s `gas_shown`) and does not loosen it.
    funds_gate(&b, &secret, fees).map_err(refused)?;
    // The exit gate, last before the chain: this ledger's lineage's anchors read now, every one held here.
    crate::exitgate::pass(&ask).map_err(refused)?;
    // The statement that touches the chain (sign, broadcast, check the echo) lives in `sign::anchor_send`:
    // the key loan is visible only in that file.
    let sent = crate::sign::anchor_send(&secret, &b.urls, b.chain, Some(b.registry.0), &b.hashes, fees, &b.backoff).map_err(refused)?;
    let tx = zikaron::hexfmt::encode(&sent.hash);
    // When the echo matches, save "submitted": after a restart this transaction is seen as waiting for its
    // receipt and is awaited, not resent.
    let (_, queue) = crate::queue::amend(&home, |q| q.mark(&b.ids, crate::queue::Step::Submitted { tx: tx.clone(), chain: b.chain }))?;
    // The receipt wait asks the place that took it first.
    Ok(Done::Submitted { tx, chain: b.chain, url: sent.taken, ids: b.ids, gas, queue })
}

/// After waiting at most this long for the receipt, the pass returns; if not yet included it stays
/// "submitted", and the next pass keeps waiting.
pub(super) fn confirm_batch(
    root: std::path::PathBuf,
    urls: Vec<String>,
    chain: u64,
    tx: String,
    ids: Vec<String>,
    gas: Option<u64>,
    wait: std::time::Duration,
) -> Result<Done, crate::fault::Fault> {
    let url = urls.first().cloned().unwrap_or_default();
    let hash: [u8; 32] = zikaron::hexfmt::decode(&tx)
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::ContentShape, tx.clone()))?;
    // Ask for the receipt endpoint by endpoint: the place that took it first, others after; if none can be
    // opened, refused by name.
    let mut opened: Vec<Box<dyn zikaron_anchor::rpc::Endpoint>> = urls.iter().filter_map(|u| crate::chainx::endpoint_at(u)).collect();
    if opened.is_empty() {
        return Err(crate::fault::Fault::known(crate::fault::Known::NoEndpoint, crate::lang::filln(crate::lang::Key::Tail216, &[&(url).to_string()])));
    }
    let mut refs: Vec<&mut dyn zikaron_anchor::rpc::Endpoint> = opened.iter_mut().map(|b| b.as_mut() as &mut dyn zikaron_anchor::rpc::Endpoint).collect();
    let confirm = zikaron_anchor::send::confirm_each(&mut refs, &hash, wait);
    // What the sent transaction actually says: read `input` back from the chain by transaction hash, never
    // reassembled here. Unreadable gives an empty string (the face then says "not read"), and the broadcast
    // has already happened, so there is no `?` on this statement.
    let calldata = {
        // Ask endpoint by endpoint and use the first that answers with this transaction (non-empty); a place
        // that does not know it yet (answers empty) moves on to the next.
        let q = zikaron::json::Value::Arr(vec![zikaron::json::Value::Str(tx.clone())]);
        let got = urls.iter().find_map(|u| {
            let ep = crate::chainx::Endpoint { chain, url: u.clone() };
            crate::chainx::ask_first(std::slice::from_ref(&ep), "eth_getTransactionByHash", &q).ok().filter(|v| !matches!(v, zikaron::json::Value::Null))
        });
        match got.ok_or(()) {
            Ok(v) => match &v {
                zikaron::json::Value::Obj(m) => m
                    .iter()
                    .find(|(k, _)| k == "input")
                    .and_then(|(_, v)| match v {
                        zikaron::json::Value::Str(x) => Some(x.clone()),
                        _ => None,
                    })
                    .unwrap_or_default(),
                _ => String::new(),
            },
            Err(_) => String::new(),
        }
    };
    let state = match &confirm {
        zikaron_anchor::send::Confirm::Included { status, block_number } => receipt_state(*status, *block_number),
        zikaron_anchor::send::Confirm::NotYet => "not yet in a block".to_string(),
        zikaron_anchor::send::Confirm::Unreachable(e) => format!("unreachable: {e}"),
    };
    let anchored = matches!(confirm, zikaron_anchor::send::Confirm::Included { status: 1, .. });
    // Removal from the queue only happens when the receipt says success (law §9.1: neither form with status
    // other than 1 is an anchor); what failed to send stays queued, so "stays queued for retry" holds because
    // no other path exists. The included transaction's block number is recorded too.
    let home = crate::home::Home::open(&root)?;
    let (dropped, queue) = match &confirm {
        zikaron_anchor::send::Confirm::Included { status: 1, block_number } => {
            let n = *block_number;
            // Removal goes only through `settle` (the only exit), and the inclusion is saved with it in one
            // write.
            crate::queue::settle(&home, &ids, true, Some(&crate::queue::Inclusion { tx: tx.clone(), chain, block: n }))?
        }
        zikaron_anchor::send::Confirm::Included { .. } => {
            crate::queue::amend(&home, |q| {
                q.mark(&ids, crate::queue::Step::Reverted { tx: tx.clone(), chain });
                0
            })?
        }
        // Not yet included and not visible: still "submitted", and the next pass keeps waiting (no resend).
        _ => crate::queue::settle(&home, &ids, false, None)?,
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
    })
}
