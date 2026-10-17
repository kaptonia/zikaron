//! Sending a batch of a home's queued anchors, one path for every end, in four stages: ready it (estimate, the
//! home's write gates, ask the chain, the balance and exit gates) and sign and send it (sign through the end's
//! port, land the batch as in flight, broadcast), both under the home's writer lock ([`anchor`]); wait for the
//! receipt and judge a batch no node holds, holding no lock ([`wait`]); and settle it, under the lock again, on
//! the queue read anew ([`settle`]). Another sender taking the lock meanwhile finds the batch in flight and leaves
//! it to its receipt, sending only the rest.
//!
//! Before anything is signed, a batch is asked about twice. What this home already has out for these entries
//! (the queue on disk records their transactions) is asked about by hash through the receipt wait and never
//! signed again ([`Sent::earlier`]); and the chain is asked whether it already holds these anchors from this key
//! (`zikaron_anchor::send::on_chain`, the rule the command line's `anchor` asks by too): held already is refused
//! by name (`OnChain`) and nothing is signed. Then the balance gate and the exit gate, the signature, and the
//! batch is recorded as in flight (`Submitted`, with its hash and nonce) BEFORE its bytes go out, so a
//! transaction that may be out is always one the queue awaits: a node's own verdict on the broadcast puts it
//! back (refused), anything else leaves it awaited, and the receipt wait asks every node by hash and judges one
//! no node holds by its nonce (void and back in the queue, or offered again).

use crate::gate;
use crate::keep;
use crate::node::{self, Endpoint, Next, NodeAddr};
use crate::{Code, End, Facts, Note, Trouble, Why};
use zikaron::json::Value;
use zikaron_anchor::rpc;
use zikaron_anchor::send::{Confirm, Fees};
use zikaron_seal::queue::{Queue, Queued, Step, RESENDS_MAX};

/// The signing port: the key stays with the end, which lends it for exactly one signature of an anchoring
/// transaction (a registry call over `hashes`, or the call data `input`) at the given fees and nonce.
pub trait Signer {
    /// The account the key sends from (`None`: the key gives no address).
    fn from(&self) -> Option<[u8; 20]>;
    /// Sign one anchoring transaction; nothing is sent.
    #[allow(clippy::too_many_arguments)]
    fn sign(&self, chain: u64, registry: Option<[u8; 20]>, hashes: &[[u8; 32]], input: Option<Vec<u8>>, fees: Fees, nonce: u64) -> Result<(Vec<u8>, [u8; 32]), rpc::Trouble>;
}

/// The anchors of a batch and what is needed to send them.
#[derive(Clone, Debug)]
pub struct Batch {
    pub ids: Vec<String>,
    pub hashes: Vec<[u8; 32]>,
    pub chain: u64,
    pub registry: [u8; 20],
    /// All of the chain's endpoints, in settings order.
    pub urls: Vec<NodeAddr>,
    /// The retry backoff delays when a node rate-limits.
    pub backoff: Vec<std::time::Duration>,
}

/// What estimating a batch needs: its chain, the call to estimate (byte-identical to the real transaction, both
/// built by the anchoring crate's `registry_calldata`) and that chain's nodes.
#[derive(Clone, Debug)]
pub struct GasAsk {
    pub chain: u64,
    pub call: Value,
    pub data: Vec<u8>,
    pub eps: Vec<Endpoint>,
}

/// One estimate: the node's gas figure, the call data given to the node, the fees read at the same nodes with
/// the gas limit the estimate sets (and the nodes left out of that reading), and the chain's time when it could
/// be read.
#[derive(Clone, Debug)]
pub struct Estimate {
    pub gas: u64,
    pub calldata: String,
    pub fees: Fees,
    pub fees_left: Vec<Why>,
    pub head_time: Option<u64>,
}

/// Estimates gas once, by the rule the command line takes too (`zikaron_anchor::send::estimate_gas`), through
/// this chain's nodes. Nodes that estimate different amounts do not disagree: the largest is taken (an estimate
/// is compared as a quantity), and it meets the ceiling like any estimate. If gas cannot be estimated the
/// transaction would revert, so it is refused by name and not sent.
pub fn estimate(a: &GasAsk) -> Result<Estimate, Trouble> {
    use zikaron_anchor::send::NoGas;
    let GasAsk { chain, call, data, eps } = a;
    let chain = *chain;
    // A failed estimate is reported as "will revert" only when the node itself refused the call, judged by the
    // same table the command line uses (`node::ask_call`, `said::refuses_the_call`). Unreachable nodes,
    // timeouts, rate limiting and other errors keep their own codes, so a person whose node is down is not told
    // the transaction will fail. The estimate is made at the lowest head every endpoint has reached
    // (`head_block`); failing to get the head is never about the call.
    let gas = zikaron_anchor::send::estimate_gas(
        || node::head_block(eps, chain).map(|(height, _)| height).map_err(|t| (t, false)),
        |params| node::ask_call(eps, "eth_estimateGas", params).map(|r| r.value),
        call.clone(),
        |(_, about_the_call): &(Trouble, bool)| !about_the_call,
    )
    .map_err(|no| match no {
        NoGas::Network((t, _)) => t,
        NoGas::Refused((t, _)) => Trouble::new(Code::GasRefused, t.why),
        NoGas::NotText(other) => Trouble::new(Code::ChainShape, Why::GasNotText(format!("{:?}", other))),
        NoGas::Unreadable(hex) => Trouble::new(Code::ChainShape, Why::GasNotQty(hex)),
        // No transaction carries more than `send::GAS_LIMIT`: an estimate above it would run out of gas on chain
        // with the fee still paid, so it is refused with the node's own number before anything is shown or
        // sent.
        NoGas::OverCap(n) => Trouble::text(Code::GasRefused, format!("{n} > {}", zikaron_anchor::send::GAS_LIMIT)),
    })?;
    // The node was reached, so fetch the chain's current time too (failure does not block the estimate).
    let head_time = node::head_time(eps, chain).ok().map(|(time, _, _)| time);
    // The fee cap comes from the chain's base fee and the gas limit from this estimate (`Fees::with_estimate`),
    // so the confirmation card, the pre-send balance gate and the signed transaction all use the same values.
    let (fees, fees_left) = node::fees(eps, chain);
    let fees = fees.with_estimate(gas);
    // Also return the call data used for estimating: the `input` of the sent transaction, read back from the
    // chain, must match it byte for byte.
    Ok(Estimate { gas, calldata: zikaron::hexfmt::encode(data), fees, fees_left, head_time })
}

/// Pre-send balance gate. The required amount comes from the same source as the transaction (`Fees::cap_wei`:
/// fee cap times gas limit, exactly what the node checks at broadcast). The balance is read now by the single
/// balance reader (`node::balance`): only this chain's nodes, at one pinned block (the lowest head all have
/// reached, so a lagging node never causes a split), with differing answers refused by name and a lone answer
/// marked single-source. An unreadable balance is refused for that reason, never assumed sufficient. The
/// chain's own refusal at broadcast remains as a backstop.
pub fn funds_gate(b: &Batch, signer: &dyn Signer, fees: Fees, note: &dyn Fn(Note)) -> Result<(), Trouble> {
    let who = signer.from().ok_or_else(|| Trouble::new(Code::KeyMalformed, Why::NoAddress))?;
    // The balance uses every endpoint for this chain: those that answer and agree count; those that do not
    // answer are skipped and reported.
    let eps: Vec<Endpoint> = b.urls.iter().map(|u| Endpoint::at(b.chain, u.clone())).collect();
    note(Note::Balance);
    let (have, _) = node::balance(&eps, b.chain, &who)?;
    funds_check(fees.cap_wei(), have)
}

/// The balance gate's comparison: a balance at least equal to the maximum spend passes (exactly enough
/// included); one wei less is refused by name with both numbers.
pub fn funds_check(need: u128, have: u128) -> Result<(), Trouble> {
    if have < need {
        return Err(Trouble::new(Code::InsufficientFunds, Why::Funds { need, have }));
    }
    Ok(())
}

/// The batch's nodes that serve its chain (each checked once per process, `node::serving`); the pre-send
/// balance, nonce and broadcast use only these. Nodes serving another chain are left out and reported as a
/// `WrongChain` refusal naming them.
pub fn serving_urls(urls: &[NodeAddr], chain: u64) -> Result<Vec<NodeAddr>, Trouble> {
    let eps: Vec<Endpoint> = urls.iter().map(|u| Endpoint::at(chain, u.clone())).collect();
    let (kept, _) = node::serving(&eps, chain)?;
    Ok(kept.into_iter().map(|e| e.url).collect())
}

/// What the chain already holds of `hashes` from `from` through `registry`, asked before anything is signed:
/// each serving node is asked by the one rule (`zikaron_anchor::send::on_chain`, from block `start` to its head),
/// and a hash any node answers with counts (each with its earliest transaction and block). Held already is refused
/// by name (`OnChain`: the hashes held, the first one's transaction and block) and nothing is signed. A node that
/// cannot say is passed over; when none can, nothing is sent on a guess, with each node's words as the evidence.
/// The refusal is named by what kept the answer away: the way to a node breaking (a handshake, a timeout, a proxy,
/// a status) by the one table, as every other read of the chain names it (`node::said_trouble`, the first such);
/// a node reached that could not say (it refused the question, or answered out of shape) is no answer
/// (`Unreachable`).
pub fn on_chain(urls: &[NodeAddr], chain: u64, registry: &[u8; 20], from: &[u8; 20], hashes: &[[u8; 32]], start: u64) -> Result<(), Trouble> {
    let mut found: Vec<zikaron_anchor::send::OnChain> = Vec::new();
    let mut heard = false;
    let mut said: Vec<Why> = Vec::new();
    let mut first: Option<Trouble> = None;
    for u in urls {
        let Some(mut ep) = node::endpoint_at(u.for_transport()) else {
            said.push(node::address_said(u.for_transport()));
            continue;
        };
        match zikaron_anchor::send::on_chain(ep.as_mut(), chain, registry, from, hashes, start) {
            Ok(got) => {
                heard = true;
                for x in got {
                    match found.iter_mut().find(|f| f.hash == x.hash) {
                        Some(f) if f.block <= x.block => {}
                        Some(f) => *f = x,
                        None => found.push(x),
                    }
                }
            }
            Err(not) => match not.trouble() {
                Some(t @ rpc::Trouble::Transport(_)) => {
                    first.get_or_insert_with(|| node::said_trouble(u.for_transport(), &t));
                    said.push(node::node_why(u.for_transport(), &t));
                }
                Some(t) => said.push(node::node_why(u.for_transport(), &t)),
                None => said.push(Why::Text(format!("{} {}", u.said(), not.evidence()))),
            },
        }
    }
    if !found.is_empty() {
        found.sort_by_key(|f| (f.block, f.hash));
        let earliest = found[0];
        let names = found.iter().map(|f| zikaron::hexfmt::encode(&f.hash)).collect();
        return Err(Trouble::new(Code::OnChain, Why::OnChain { names, tx: zikaron::hexfmt::encode(&earliest.tx), block: earliest.block }));
    }
    if !heard {
        return Err(match first {
            Some(t) => t.retold(Why::Each(said)),
            None => Trouble::new(Code::Unreachable, Why::Each(said)),
        });
    }
    Ok(())
}

/// A batch put out: its last transaction, its chain, the node that took it, its entries, the gas estimated for
/// it and the queue as written. `earlier`: these entries were already out from this home (the queue records their
/// transactions), so nothing new was signed and this is that batch, handed to the receipt wait to be asked about
/// by hash.
#[derive(Clone, Debug)]
pub struct Sent {
    pub tx: String,
    pub chain: u64,
    pub url: NodeAddr,
    pub ids: Vec<String>,
    pub gas: Option<u64>,
    pub queue: Queue,
    pub earlier: bool,
}

/// The batch out from this home for any of `ids`, as the queue on disk records it: its transactions (oldest
/// first), its chain and every entry awaiting them. `None` when none of `ids` has a transaction out.
pub fn out_for(q: &Queue, ids: &[String]) -> Option<(Vec<String>, u64, Vec<String>)> {
    let (txs, chain) = ids.iter().find_map(|id| q.step_of(id).and_then(Step::awaited))?;
    let all: Vec<String> = q.items.iter().filter(|x| x.step.awaited().is_some_and(|(t, c)| t == txs && c == chain)).map(|x| x.id.clone()).collect();
    Some((txs, chain, all))
}

/// Sends one batch for `end`: earlier, the home's write gates, on chain, balance, exit gate, sign, land, broadcast
/// (the module's head): the first two stages of a send, which the end runs holding the home's writer lock. It goes
/// only as far as "submitted": once the batch is landed as in flight and a node took the bytes, `Sent` is
/// returned and the end starts the receipt wait ([`wait`], holding no lock) and writes what it heard ([`settle`],
/// holding the lock again). A refusal before the landing records the batch as
/// refused, with the refusal's code (entries in flight are never overwritten, `Queue::mark`). `start` is the
/// block the chain is asked from whether it already holds the anchors (the home's scan window).
#[allow(clippy::too_many_arguments)]
pub fn anchor<E: End + Facts>(end: &E, b: Batch, signer: &dyn Signer, gas: Option<u64>, fees: Fees, ask: gate::Ask, start: u64) -> Result<Sent, E::Fail> {
    let mut b = b;
    let refused = |f: E::Fail, ids: &[String]| -> E::Fail {
        let said = E::code_of(&f).unwrap_or_default();
        let _ = keep::amend(end, |q| q.mark(ids, Step::Refused { said }));
        f
    };
    // What this home already has out for these entries is asked about by hash (the receipt wait), never signed
    // again.
    let disk = keep::read(end)?;
    if let Some(out) = out_for(&disk, &b.ids) {
        return Ok(earlier(out, &b, gas, disk));
    }
    // The home's write gates, from what is on disk, before the chain is asked anything.
    crate::shut::check(end).map_err(|f| refused(f, &b.ids))?;
    b.urls = serving_urls(&b.urls, b.chain).map_err(|t| refused(t.into(), &b.ids))?;
    let from = signer.from().ok_or_else(|| refused(Trouble::new(Code::KeyMalformed, Why::NoAddress).into(), &b.ids))?;
    // What the chain already holds is not sent again, whichever side put it there.
    on_chain(&b.urls, b.chain, &b.registry, &from, &b.hashes, start).map_err(|t| refused(t.into(), &b.ids))?;
    // Pre-send balance gate: if the balance does not cover fee cap times gas, refuse by name with both amounts
    // and broadcast nothing. It runs after the end's show-before-send gate, not instead of it.
    funds_gate(&b, signer, fees, &|n| end.note(n)).map_err(|t| refused(t.into(), &b.ids))?;
    // The exit gate, last before the chain: every anchor of this ledger's lineage, read now, must be held here.
    let pass = gate::pass(&ask, end).map_err(|f| refused(f, &b.ids))?;
    end.note(Note::Stage(0));
    let (raw, hash, nonce) = sign_new(signer, &b, fees).map_err(|t| refused(t.into(), &b.ids))?;
    let tx = zikaron::hexfmt::encode(&hash);
    // Landed as in flight before its bytes go: after a restart this transaction is awaited, never resent. Only
    // when every entry of the batch is still not out; one another sender put out meanwhile sends nothing now.
    let (out, queue) = keep::amend(end, |q| match out_for(q, &b.ids) {
        Some(out) => Some(out),
        None => {
            q.mark(&b.ids, Step::Submitted { tx: tx.clone(), chain: b.chain, nonce: Some(nonce) });
            None
        }
    })?;
    if let Some(out) = out {
        return Ok(earlier(out, &b, gas, queue));
    }
    match cast(&pass, &b.urls, b.chain, &raw, hash, &b.backoff, None, &|n| end.note(n)) {
        Ok(c) => Ok(Sent { tx, chain: b.chain, url: c.taken, ids: b.ids, gas, queue, earlier: false }),
        Err(t) => {
            // A node's own verdict (insufficient funds, nonce used, underpriced, …) means the bytes were not
            // accepted: the batch goes back, refused with that code. Any other failure (no node answered, a
            // network error after the bytes were sent) may leave them in a pool: the batch stays awaited, and the
            // receipt wait asks every node by hash.
            if node::next_after(&t) == Next::Stop {
                let f: E::Fail = t.into();
                let said = E::code_of(&f).unwrap_or_default();
                let _ = keep::amend(end, |q| q.moved_by(&b.ids, std::slice::from_ref(&tx), b.chain, Step::Refused { said }));
                return Err(f);
            }
            Err(t.into())
        }
    }
}

/// The batch already out, as [`Sent`] for the receipt wait.
fn earlier((txs, chain, ids): (Vec<String>, u64, Vec<String>), b: &Batch, gas: Option<u64>, queue: Queue) -> Sent {
    let url = b.urls.first().cloned().unwrap_or_else(|| NodeAddr::new(String::new()));
    Sent { tx: txs.last().cloned().unwrap_or_default(), chain, url, ids, gas, queue, earlier: true }
}

/// Sign a new batch once: the pending nonce is asked of every endpoint at once, each with the patience table
/// (`zikaron_anchor::patience`: a rate limit is waited out there, never here), and the largest is taken (a node
/// that is behind would hand out a nonce already used). An endpoint that did not answer is skipped; a node's
/// substantive verdict refuses at once; if none answers, the last answer is returned.
fn sign_new(signer: &dyn Signer, b: &Batch, fees: Fees) -> Result<(Vec<u8>, [u8; 32], u64), Trouble> {
    let urls = &b.urls;
    // The first node in its safe-to-display form, as a refusal's evidence names it.
    let first = urls.first().map(NodeAddr::said).unwrap_or_default();
    let open = |url: &NodeAddr| node::endpoint_at(url.for_transport()).ok_or_else(|| Trouble::new(Code::SettingsShape, node::address_said(url.for_transport())));
    let mut last: Option<Trouble> = None;
    let Some(from) = signer.from() else {
        return Err(last.unwrap_or_else(|| Trouble::text(Code::NoEndpoint, first)));
    };
    let params = zikaron_anchor::send::nonce_params(&from);
    let mut nodes: Vec<(String, Box<dyn rpc::Endpoint + Send>)> = Vec::new();
    let mut words: Vec<(usize, Trouble)> = Vec::new();
    for (i, url) in urls.iter().enumerate() {
        match open(url) {
            Ok(h) => nodes.push((url.for_transport().to_string(), h)),
            Err(t) => words.push((i, t)),
        }
    }
    let answers = zikaron_anchor::endpoints::ask_each(nodes, "eth_getTransactionCount", &params);
    match zikaron_anchor::judge::judge("eth_getTransactionCount", &params, answers) {
        Ok(j) => {
            let taker = j.sources.first().cloned().unwrap_or_default();
            match j.value.as_str().and_then(node::wei).and_then(|n| u64::try_from(n).ok()) {
                Some(nonce) => match signer.sign(b.chain, Some(b.registry), &b.hashes, None, fees, nonce) {
                    Ok((raw, hash)) => return Ok((raw, hash, nonce)),
                    Err(t) => return Err(node::said_trouble(&taker, &t)),
                },
                None => last = Some(nonce_unread(&taker)),
            }
        }
        Err(zikaron_anchor::judge::NoReading::NoneAnswered(missing)) => {
            for (url, m) in missing {
                let said = match m {
                    zikaron_anchor::judge::Missing::Trouble(t) => node::said_trouble(&url, &t),
                    zikaron_anchor::judge::Missing::Unreadable(_) => nonce_unread(&url),
                };
                let i = urls.iter().position(|u| u.for_transport() == url).unwrap_or(usize::MAX);
                words.push((i, said));
            }
            words.sort_by_key(|(i, _)| *i);
            if let Some((_, t)) = words.iter().find(|(_, t)| node::next_after(t) == Next::Stop) {
                return Err(t.clone());
            }
            last = words.pop().map(|(_, t)| t);
        }
        // Taking the maximum, the answers never disagree.
        Err(_) => last = Some(Trouble::text(Code::Unreachable, first.clone())),
    }
    Err(last.unwrap_or_else(|| Trouble::text(Code::NoEndpoint, first)))
}

/// A node whose answer to the nonce question did not read: the node answered, so it is no connection's failure,
/// but there is no nonce to sign with (`Unreachable`, as no answer); the evidence names the node.
fn nonce_unread(url: &str) -> Trouble {
    Trouble::new(Code::Unreachable, Why::NonceUnread { node: zikaron_net::sayable(url) })
}

/// Where a broadcast went: hash, how many submissions were made, which node took it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cast {
    pub hash: [u8; 32],
    /// How many times the transaction (`eth_sendRawTransaction`) was submitted in total (including backoff
    /// retries at the same place).
    pub asked: usize,
    /// The endpoint that took it.
    pub taken: NodeAddr,
}

/// Submit signed bytes endpoint by endpoint (every anchoring transaction goes out here, once per signing), with
/// the exit gate's [`gate::Pass`]: there is no way to the chain except through the gate.
///
/// 1. Submit the same signed bytes endpoint by endpoint; a matching echo is success. Network-kind refusals
///    (rate limit, no answer, timeout, bad answer, access refused: [`Code::NETWORK`]) move to the next endpoint
///    (a rate limit first retries the same one per `backoff`). The nonce is fixed and the hash identical, so
///    there is no double spend.
/// 2. A node's substantive verdict (insufficient balance, nonce used, underpriced, execution reverted…) would be
///    the same everywhere, so it is returned at once without asking further; one that answers "already pending"
///    or "nonce used" is first asked by hash whether it knows this very transaction.
/// 3. Only when no endpoint took it is it refused, with the last answer.
#[allow(clippy::too_many_arguments)]
pub fn cast(_pass: &gate::Pass, urls: &[NodeAddr], chain: u64, raw: &[u8], hash: [u8; 32], backoff: &[std::time::Duration], last: Option<Trouble>, note: &dyn Fn(Note)) -> Result<Cast, Trouble> {
    // The first node in its safe-to-display form, as a refusal's evidence names it.
    let first = urls.first().map(NodeAddr::said).unwrap_or_default();
    let mut last = last;
    let open = |url: &NodeAddr| node::endpoint_at(url.for_transport()).ok_or_else(|| Trouble::new(Code::SettingsShape, node::address_said(url.for_transport())));
    let waits: Vec<std::time::Duration> = std::iter::once(std::time::Duration::ZERO).chain(backoff.iter().copied()).collect();
    note(Note::Stage(1));
    let mut asked = 0usize;
    // An earlier endpoint already received these bytes (it answered with a network-kind refusal, and the
    // bytes may be in its pool).
    let mut handed = false;
    for url in urls {
        let mut http = match open(url) {
            Ok(h) => h,
            Err(t) => {
                last = Some(t);
                continue;
            }
        };
        for w in &waits {
            if !w.is_zero() {
                std::thread::sleep(*w);
            }
            asked += 1;
            match zikaron_anchor::send::submit(http.as_mut(), raw, &hash) {
                Ok(()) => return Ok(Cast { hash, asked, taken: url.clone() }),
                Err(e) => {
                    // What the node said, classified only in `node::said_trouble`.
                    let t = node::said_trouble(url.for_transport(), &e);
                    // "Already pending" or "nonce used": ask once by hash; if a node knows the transaction it was
                    // taken (then wait for the receipt), otherwise refuse as before.
                    if node::already_taken(&t) && node::tx_known(urls, chain, &hash) {
                        return Ok(Cast { hash, asked, taken: url.clone() });
                    }
                    match node::next_after(&t) {
                        Next::Retry => last = Some(t),
                        Next::NextEndpoint => {
                            handed = true;
                            last = Some(t);
                            break;
                        }
                        Next::Stop => return Err(t),
                    }
                }
            }
        }
    }
    // Submitted but no endpoint answered (timeout, disconnect): the bytes may be in some pool. Ask once by hash;
    // known means taken (the receipt decides), unknown means refuse. Refusing outright without the landing before
    // would make the next pass anchor again with a new nonce, and if the earlier transaction was also included the
    // batch would be anchored twice.
    if handed && node::tx_known(urls, chain, &hash) {
        let Some(taken) = urls.first().cloned() else { return Err(last.unwrap_or_else(|| Trouble::text(Code::NoEndpoint, first))) };
        return Ok(Cast { hash, asked, taken });
    }
    Err(last.unwrap_or_else(|| Trouble::text(Code::NoEndpoint, first)))
}

/// A batch's transaction not included by the end of a receipt wait: every transaction of the batch, oldest
/// first, and the latest as the chain holds it (nonce, recipient, call data and fees), with the current price.
/// Read where the wait ended; the "resend with higher fees" offer reads only this.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stuck {
    pub txs: Vec<String>,
    pub chain: u64,
    /// The account the transactions were sent from (a resend is signed by the same key or refused).
    pub from: [u8; 20],
    pub nonce: u64,
    pub to: [u8; 20],
    pub input: Vec<u8>,
    /// The last transaction's fee cap, priority fee and gas limit.
    pub sent: Fees,
    /// What may be done now.
    pub offer: Offer,
    /// The nodes the current price was not read from because they serve another chain, each named.
    pub left: Vec<Why>,
}

/// Whether a stuck batch may be resent with higher fees, and at what.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Offer {
    /// The current price (the head's base fee) is above the last transaction's cap: a resend carries these fees
    /// (the current fee rule, at least the pools' replacement bump above the last, `Fees::replacing`).
    Resend { fees: Fees, base_now: u64 },
    /// The current price is not above the cap: the transaction can still be included as is; waiting continues.
    NotAbove { base_now: u64 },
    /// The current price could not be read: nothing is offered (waiting continues).
    PriceUnread,
    /// Already resent [`RESENDS_MAX`] times: waiting continues, nothing more is offered.
    Spent,
    /// No node holds any transaction of the batch any more (dropped from the pools) and its nonce is still
    /// unused on chain: the batch as recorded (its entries, at its nonce) may be sent again at the current price
    /// with these fees (gas as the node estimates now; nothing is replaced, so no bump is needed).
    Unheld { fees: Fees, base_now: u64 },
}

/// Resends a stuck batch with higher fees, with the fees the end showed (`fees`, never above what was shown):
/// ask the chain whether it already holds these anchors, check the balance for the new cap, pass the exit gate,
/// sign the same transaction again (its nonce, recipient and call data, `Stuck`) with the same key (another is
/// refused by name), then record the new hash with the batch (`Resent`) BEFORE the single broadcast, so any
/// resend that goes out is one the queue watches. If a node refuses the broadcast the hash is removed again; if
/// the bytes may be out it stays; a refusal before broadcast changes nothing. Transactions already out stay
/// awaited either way (never marked refused, since the first may still be included), and a recorded hash no
/// node holds is skipped by the wait.
#[allow(clippy::too_many_arguments)]
pub fn resend<E: End + Facts>(end: &E, b: Batch, stuck: Stuck, fees: Fees, signer: &dyn Signer, ask: gate::Ask, start: u64) -> Result<Sent, E::Fail> {
    let mut b = b;
    crate::shut::check(end)?;
    b.urls = serving_urls(&b.urls, b.chain)?;
    // What the chain already holds is not sent again, whichever side put it there.
    on_chain(&b.urls, b.chain, &b.registry, &stuck.from, &b.hashes, start)?;
    funds_gate(&b, signer, fees, &|n| end.note(n))?;
    let pass = gate::pass(&ask, end)?;
    // The same key that sent the batch, or nothing is signed.
    if signer.from() != Some(stuck.from) {
        return Err(Trouble::text(Code::AddressMismatch, zikaron::hexfmt::encode(&stuck.from)).into());
    }
    end.note(Note::Stage(0));
    let first = b.urls.first().map(|u| u.for_transport().to_string()).unwrap_or_default();
    let (raw, hash) = signer.sign(stuck.chain, Some(stuck.to), &[], Some(stuck.input.clone()), fees, stuck.nonce).map_err(|t| node::said_trouble(&first, &t))?;
    let tx = zikaron::hexfmt::encode(&hash);
    let before = if stuck.txs.len() == 1 {
        Step::Submitted { tx: stuck.txs[0].clone(), chain: b.chain, nonce: Some(stuck.nonce) }
    } else {
        Step::Resent { txs: stuck.txs.clone(), chain: b.chain, nonce: Some(stuck.nonce) }
    };
    // Signing the same bytes again (a batch no node holds, offered at its original price) gives the same
    // transaction: broadcast it again but watch it once.
    let mut txs = stuck.txs.clone();
    if !txs.contains(&tx) {
        txs.push(tx.clone());
    }
    let after = match txs.as_slice() {
        [one] => Step::Submitted { tx: one.clone(), chain: b.chain, nonce: Some(stuck.nonce) },
        _ => Step::Resent { txs: txs.clone(), chain: b.chain, nonce: Some(stuck.nonce) },
    };
    let (_, queue) = keep::amend(end, |q| q.moved_by(&b.ids, &stuck.txs, b.chain, after))?;
    let c = match cast(&pass, &b.urls, b.chain, &raw, hash, &b.backoff, None, &|n| end.note(n)) {
        Ok(c) => c,
        Err(t) => {
            // A node's own verdict (underpriced, nonce used, …) means the bytes were not accepted, so the hash is
            // removed. Any other failure (no node answered, a network error after the bytes were sent) may leave
            // them in a mempool: the hash stays watched, and the wait skips it while no node holds it.
            if node::next_after(&t) == Next::Stop {
                let _ = keep::amend(end, |q| q.moved_by(&b.ids, &txs, b.chain, before));
            }
            return Err(t.into());
        }
    };
    Ok(Sent { tx, chain: b.chain, url: c.taken, ids: b.ids, gas: None, queue, earlier: false })
}

/// What one receipt wait found, as evidence: included (its status and block), not yet, out of sight (the last
/// words heard), split between nodes, void (held by no node, its nonce used by another transaction), or held by
/// no node with its nonce unused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Receipt {
    Included { status: u64, block: u64 },
    NotYet,
    Unheard(String),
    Split { has: Vec<String>, not_yet: Vec<String> },
    Void,
    Unused,
}

/// What one receipt wait heard ([`wait`]), nothing written yet: the batch asked about (every transaction, oldest
/// first, and its entries) and what the chain said, for [`settle`] to write under the writer's lock.
#[derive(Clone, Debug)]
pub struct Heard {
    /// The transaction included, or the batch's latest.
    pub tx: String,
    pub chain: u64,
    pub txs: Vec<String>,
    pub ids: Vec<String>,
    pub confirmed: bool,
    pub receipt: Receipt,
    pub gas: Option<u64>,
    pub calldata: String,
    pub stuck: Option<Stuck>,
    pub voided: bool,
    pub waited: u64,
}

/// A batch's receipt wait ended. The transaction hash is always present: the bytes were broadcast and must be
/// traceable.
#[derive(Clone, Debug)]
pub struct Landed {
    pub tx: String,
    pub chain: u64,
    /// Whether the transaction succeeded (`zikaron/1` §9.1: a status other than 1 is no anchor in either form).
    pub confirmed: bool,
    pub receipt: Receipt,
    /// How many were sent in this batch, and how many left the queue.
    pub sent: usize,
    pub dropped: usize,
    /// The queue table on disk after this wait, unchanged.
    pub queue: Vec<Queued>,
    /// Gas estimated by the node before sending.
    pub gas: Option<u64>,
    /// The call data of the transaction actually sent, read back from the chain by its hash (never rebuilt
    /// here). Empty when it cannot be read.
    pub calldata: String,
    /// Not included by the end of this wait: the batch's latest transaction as the chain holds it and the
    /// current price. `None` once included (successfully or not), or when the wait ended with no node seeing it.
    pub stuck: Option<Stuck>,
    /// No transaction of the batch was included, none is held by a node, and its nonce is used on chain by
    /// another transaction: the batch is void and its entries are back in the queue.
    pub voided: bool,
    /// How long the receipt wait actually ran, in whole seconds (measured, not the wait it was given).
    pub waited: u64,
}

/// The account a batch is sent from and the registry it calls. Used to judge a batch no node holds (via the
/// account's on-chain nonce) and to resend it.
#[derive(Clone, Copy, Debug)]
pub struct Sender {
    pub from: [u8; 20],
    pub registry: [u8; 20],
}

/// The receipt wait (a send's third stage): waits at most `wait` for the receipt and returns what was heard,
/// writing nothing and holding no lock, so another sender may send the rest of the queue meanwhile; [`settle`]
/// writes it. If not yet included the batch stays submitted (or resent) and the next wait asks again.
///
/// Every transaction of the batch is awaited (`txs`, oldest first: one, or more after fee-bump resends),
/// sharing the wait. They share one nonce, so at most one can be included, and the first found included
/// counts. If none is included by the end, the latest is read as the chain holds it, with the current price
/// ([`stuck_of`]): this is what the "resend with higher fees" offer reads. If none is included and no node holds
/// any (dropped from every mempool, or the nonce used by another transaction), the batch is judged by the
/// sending account's on-chain nonce against its own ([`unheld`]): voided and requeued, or offered again as
/// recorded at its nonce and the current price. Whether a node still holds the transaction never decides
/// whether action is possible: a batch no node holds is exactly the one that needs it.
#[allow(clippy::too_many_arguments)]
pub fn wait(
    urls: Vec<NodeAddr>,
    chain: u64,
    txs: Vec<String>,
    ids: Vec<String>,
    gas: Option<u64>,
    wait: std::time::Duration,
    backoff: &[std::time::Duration],
    sender: Option<Sender>,
    nonce: Option<u64>,
) -> Result<Heard, Trouble> {
    let url = urls.first().map(|u| u.for_transport().to_string()).unwrap_or_default();
    let mut hashes: Vec<[u8; 32]> = Vec::with_capacity(txs.len());
    for tx in &txs {
        let h: [u8; 32] = zikaron::hexfmt::decode(tx).and_then(|b| b.try_into().ok()).ok_or_else(|| Trouble::text(Code::ContentShape, tx.clone()))?;
        hashes.push(h);
    }
    if hashes.is_empty() {
        return Err(Trouble::text(Code::QueueEmpty, ids.join(" ")));
    }
    // Ask each endpoint for the receipt, starting with the one that accepted the transaction; if none can be
    // opened, refuse by name.
    let mut opened: Vec<Box<dyn rpc::Endpoint + Send>> = urls.iter().filter_map(|u| node::endpoint_at(u.for_transport())).collect();
    if opened.is_empty() {
        return Err(Trouble::new(Code::SettingsShape, node::address_said(&url)));
    }
    let mut refs: Vec<&mut dyn rpc::Endpoint> = opened.iter_mut().map(|b| b.as_mut() as &mut dyn rpc::Endpoint).collect();
    // One wait shared between the transactions; the first included counts. Otherwise report the most
    // informative result: some nodes with a receipt and some without, then "not yet", then unreachable.
    let share = wait / hashes.len() as u32;
    let rank = |c: &Confirm| match c {
        Confirm::Included { .. } => 3,
        Confirm::Split { .. } => 2,
        Confirm::NotYet => 1,
        Confirm::Unreachable(_) => 0,
    };
    let mut confirm: Option<Confirm> = None;
    let mut tx = txs.last().cloned().unwrap_or_default();
    let began = std::time::Instant::now();
    for (h, t) in hashes.iter().zip(&txs) {
        let c = zikaron_anchor::send::confirm_each(&mut refs, h, share, backoff);
        let included = matches!(c, Confirm::Included { .. });
        if confirm.as_ref().is_none_or(|was| rank(&c) > rank(was)) {
            confirm = Some(c);
            if included {
                tx = t.clone();
                break;
            }
        }
    }
    let confirm = confirm.unwrap_or(Confirm::NotYet);
    let waited = began.elapsed().as_secs();
    // What the sent transaction actually contains: `input` read back from the chain by hash, never rebuilt
    // here. If unreadable it is empty; the broadcast already happened, so this must not fail the call.
    let calldata = node::tx_as_held(&urls, chain, &tx).as_ref().and_then(|v| text_member(v, "input")).unwrap_or_default();
    let receipt = match &confirm {
        Confirm::Included { status, block_number } => Receipt::Included { status: *status, block: *block_number },
        Confirm::NotYet => Receipt::NotYet,
        Confirm::Unreachable(e) => Receipt::Unheard(e.to_string()),
        // Some nodes have the receipt and others not yet, even after re-asking: not treated as anchored on the
        // word of some nodes; it stays submitted and the next call asks again.
        Confirm::Split { has, not_yet } => Receipt::Split { has: has.clone(), not_yet: not_yet.clone() },
    };
    // Only a receipt reporting success anchors (a status other than 1 is not an anchor).
    let anchored = matches!(confirm, Confirm::Included { status: 1, .. });
    // If not included by the end of this wait: the latest transaction of the batch a node still holds (later
    // ones recorded but never accepted, or dropped from mempools, are skipped).
    let held = match &confirm {
        Confirm::NotYet => Some(txs.iter().rev().find_map(|t| node::tx_as_held(&urls, chain, t))),
        _ => None,
    };
    // Held by no node: judge it by the account's on-chain nonce (`unheld`) rather than waiting silently.
    let unheld = match (&confirm, &held, sender) {
        (Confirm::NotYet, Some(None), Some(who)) => unheld(&urls, chain, &txs, &ids, nonce, who, &mut refs),
        _ => Unheld::Wait,
    };
    let receipt = match &unheld {
        Unheld::Void => Receipt::Void,
        Unheld::Again(_) => Receipt::Unused,
        Unheld::Wait => receipt,
    };
    let voided = matches!(unheld, Unheld::Void);
    // What can be done now, from the transaction a node holds and the current price; if no node holds it and
    // its nonce is unused, the batch as recorded at the current price.
    let stuck = match unheld {
        Unheld::Again(s) => Some(s),
        _ => held.flatten().and_then(|v| stuck_of(&urls, chain, &txs, &v)),
    };
    Ok(Heard { tx, chain, txs, ids, confirmed: anchored, receipt, gas, calldata, stuck, voided, waited })
}

/// The receipt written into the queue (a send's last stage), under the writer's lock the end holds again: the
/// queue is read anew, and only entries still awaiting this batch change, so a batch another sender settled
/// meanwhile is left as it stands. Included with status 1: those still awaiting a set of transactions holding the
/// included one leave the queue, recorded as anchored at that block (removal goes only through here and
/// `keep::settle`); included with another status: reverted; void: back in the queue (`Queued`, sendable again),
/// and answered as void unless the entries now await a later set holding those transactions (resent by another
/// sender meanwhile: no void of this batch, `Landed::voided` false); otherwise nothing is written (still in flight,
/// the next wait asks again).
pub fn settle<E: End>(end: &E, h: Heard) -> Result<Landed, E::Fail> {
    let Heard { tx, chain, txs, ids, confirmed, receipt, gas, calldata, stuck, mut voided, waited } = h;
    let (dropped, queue) = match &receipt {
        Receipt::Included { status: 1, block } => keep::amend(end, |q| {
            let ours: Vec<String> = ids.iter().filter(|id| q.step_of(id).and_then(Step::awaited).is_some_and(|(t, c)| c == chain && t.contains(&tx))).cloned().collect();
            q.included_out(&ours, &tx, chain, *block)
        })?,
        Receipt::Included { .. } => keep::amend(end, |q| {
            q.moved_by(&ids, &txs, chain, Step::Reverted { tx: tx.clone(), chain });
            0
        })?,
        _ if voided => {
            let ((moved, resent), queue) = keep::amend(end, |q| {
                let moved = q.moved_by(&ids, &txs, chain, Step::Queued);
                // Judged void on the transactions asked about. Entries that now await a later set holding one of
                // them (another sender resent them meanwhile, and that may well be in) are no void of these; entries
                // already back (another wait on the same batch settled first) are.
                let resent = ids.iter().any(|id| q.step_of(id).and_then(Step::awaited).is_some_and(|(t, c)| c == chain && t.iter().any(|x| txs.contains(x))));
                (moved, resent)
            })?;
            voided = moved > 0 || !resent;
            (0, queue)
        }
        _ => (0, keep::read(end)?),
    };
    Ok(Landed { tx, chain, confirmed, receipt, sent: ids.len(), dropped, queue: queue.items, gas, calldata, stuck, voided, waited })
}

/// What becomes of a batch no node holds ([`unheld`]).
enum Unheld {
    /// Its nonce is used on chain and none of its transactions was included: void, back in the queue.
    Void,
    /// Its nonce is unused: the batch as recorded may be sent again (the offer).
    Again(Stuck),
    /// Not decided (on-chain nonce unread, or a receipt found on re-asking): keep waiting.
    Wait,
}

/// Judges a batch whose transactions no node holds by comparing the sending account's on-chain nonce with the
/// batch's own (`nonce`, recorded at signing; a batch held without one takes the account's next nonce, and is
/// offered again rather than voided on a guess):
/// - **used** (every answering node's nonce is past it): each transaction's receipt is asked once more (a node
///   past the nonce has the block, and the receipt if it was this batch's); if all are "not yet" the batch is
///   void, otherwise it waits.
/// - **unused**: the batch as recorded here (its entries, the registry call over them, at its nonce) is offered
///   again at the current price ([`again_of`]).
///
/// The rule is the anchoring crate's (`zikaron_anchor::send::unheld`, also used by the command line's `anchor`
/// on a rerun); this side supplies the queries (every node serving the chain).
#[allow(clippy::too_many_arguments)]
fn unheld(urls: &[NodeAddr], chain: u64, txs: &[String], ids: &[String], nonce: Option<u64>, who: Sender, refs: &mut [&mut dyn rpc::Endpoint]) -> Unheld {
    let on_chain = chain_nonce(urls, chain, &who.from);
    let mine = nonce.or(on_chain).unwrap_or_default();
    let none_included = || {
        txs.iter().all(|t| {
            let h: Option<[u8; 32]> = zikaron::hexfmt::decode(t).and_then(|b| b.try_into().ok());
            h.is_some_and(|h| matches!(zikaron_anchor::send::confirm_each(refs, &h, std::time::Duration::ZERO, &[]), Confirm::NotYet))
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
fn chain_nonce(urls: &[NodeAddr], chain: u64, from: &[u8; 20]) -> Option<u64> {
    let eps: Vec<Endpoint> = urls.iter().map(|u| Endpoint::at(chain, u.clone())).collect();
    let (eps, _) = node::serving(&eps, chain).ok()?;
    let nodes: Vec<(String, Box<dyn rpc::Endpoint + Send>)> = eps.iter().filter_map(|e| node::endpoint_at(e.url.for_transport()).map(|h| (e.url.for_transport().to_string(), h))).collect();
    let params = Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(from)), Value::Str("latest".into())]);
    zikaron_anchor::endpoints::ask_each(nodes, "eth_getTransactionCount", &params)
        .into_iter()
        .filter_map(|(_, got)| match got.ok().and_then(|w| zikaron_anchor::wire::to_core(&w)) {
            Some(Value::Str(x)) => node::wei(&x).and_then(|n| u64::try_from(n).ok()),
            _ => None,
        })
        .min()
}

/// Offers a batch no node holds again, as recorded here: the registry call over its entries, to the registry,
/// from the account, at `nonce`, with the current price and a fresh gas estimate (nothing is replaced, so no
/// bump over the last). Once resent [`RESENDS_MAX`] times nothing more is offered; if the price or gas cannot be
/// read, nothing is offered and the wait goes on.
fn again_of(urls: &[NodeAddr], chain: u64, txs: &[String], ids: &[String], nonce: u64, who: Sender) -> Option<Stuck> {
    let input = zikaron_anchor::send::registry_calldata(&Queue::hashes(ids).ok()?);
    let eps: Vec<Endpoint> = urls.iter().map(|u| Endpoint::at(chain, u.clone())).collect();
    let now = node::fee_reading(&eps, chain);
    let gas = zikaron_anchor::send::estimate_gas(
        || node::head_block(&eps, chain).map(|(height, _)| height).map_err(|t| (t, false)),
        |params| node::ask_call(&eps, "eth_estimateGas", params).map(|r| r.value),
        zikaron_anchor::send::estimate_call(&who.from, &who.registry, &input),
        |(_, about_the_call): &(Trouble, bool)| !about_the_call,
    )
    .ok();
    let offer = match (txs.len() > RESENDS_MAX, now.base, gas) {
        (true, _, _) => Offer::Spent,
        (false, Some(base_now), Some(gas)) => Offer::Unheld { fees: now.fees.with_estimate(gas), base_now },
        (false, _, _) => Offer::PriceUnread,
    };
    let sent = match &offer {
        Offer::Unheld { fees, .. } => *fees,
        _ => now.fees,
    };
    Some(Stuck { txs: txs.to_vec(), chain, from: who.from, nonce, to: who.registry, input, sent, offer, left: now.left })
}

fn text_member(v: &Value, k: &str) -> Option<String> {
    match v {
        Value::Obj(m) => m.iter().find(|(n, _)| n == k).and_then(|(_, v)| match v {
            Value::Str(x) => Some(x.clone()),
            _ => None,
        }),
        _ => None,
    }
}

/// A batch not included by the end of a wait: its last transaction as the chain holds it (`held`) and what can
/// be done now. `None` if the transaction's fields cannot be read (nothing is offered then).
fn stuck_of(urls: &[NodeAddr], chain: u64, txs: &[String], held: &Value) -> Option<Stuck> {
    let qty = |k: &str| text_member(held, k).as_deref().and_then(node::wei).and_then(|n| u64::try_from(n).ok());
    let addr = |k: &str| text_member(held, k).as_deref().and_then(zikaron::hexfmt::decode).and_then(|b| <[u8; 20]>::try_from(b).ok());
    let input = text_member(held, "input").as_deref().and_then(zikaron::hexfmt::decode)?;
    let sent = Fees { max_fee: qty("maxFeePerGas")?, priority: qty("maxPriorityFeePerGas")?, gas_limit: qty("gas")?, from_chain: true };
    let (from, nonce, to) = (addr("from")?, qty("nonce")?, addr("to")?);
    let (offer, left) = if txs.len() > RESENDS_MAX {
        (Offer::Spent, Vec::new())
    } else {
        let eps: Vec<Endpoint> = urls.iter().map(|u| Endpoint::at(chain, u.clone())).collect();
        let now = node::fee_reading(&eps, chain);
        let offer = match now.base {
            Some(base_now) if base_now > sent.max_fee => Offer::Resend { fees: Fees::replacing(sent, now.fees), base_now },
            Some(base_now) => Offer::NotAbove { base_now },
            None => Offer::PriceUnread,
        };
        (offer, now.left)
    };
    Some(Stuck { txs: txs.to_vec(), chain, from, nonce, to, input, sent, offer, left })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What this home has out for a batch, one form per line: nothing out (`None`); one of the asked entries out
    /// (that batch's transactions, chain and every entry awaiting them, asked or not); a resent batch (all its
    /// transactions); another chain's batch with the same transactions is not the same batch.
    #[test]
    fn what_a_home_has_out_is_read_by_the_transactions_awaited() {
        let id = |n: u8| zikaron::hexfmt::encode(&[n; 32]);
        let tx = |n: u8| zikaron::hexfmt::encode(&[0xf0 | n; 32]);
        let mut q = Queue::default();
        for n in 1..=5 {
            let _ = q.push(&id(n), n as u64);
        }
        assert_eq!(out_for(&q, &[id(1), id(2)]), None, "nothing out");
        q.mark(&[id(2), id(3)], Step::Submitted { tx: tx(1), chain: 7, nonce: Some(0) });
        q.mark(&[id(4)], Step::Submitted { tx: tx(1), chain: 8, nonce: Some(0) });
        assert_eq!(out_for(&q, &[id(1), id(2)]), Some((vec![tx(1)], 7, vec![id(2), id(3)])), "the batch out, every entry of it");
        q.mark(&[id(5)], Step::Resent { txs: vec![tx(2), tx(3)], chain: 7, nonce: Some(1) });
        assert_eq!(out_for(&q, &[id(5)]), Some((vec![tx(2), tx(3)], 7, vec![id(5)])), "a resent batch, all its transactions");
        assert_eq!(out_for(&q, &[id(4)]), Some((vec![tx(1)], 8, vec![id(4)])), "the same transaction on another chain is another batch");
    }

    /// A queue held in memory, for the settling stage.
    struct Mem(std::cell::RefCell<Queue>);

    /// The memory end words a sealed-layer trouble as the sender's own refusal, its code's spelling the evidence.
    impl From<zikaron_seal::Trouble> for Trouble {
        fn from(t: zikaron_seal::Trouble) -> Trouble {
            Trouble::text(Code::SettingsShape, format!("{:?} {}", t.code, t.tail))
        }
    }

    impl End for Mem {
        type Fail = Trouble;
        fn code_of(f: &Trouble) -> Option<String> {
            Some(f.code.as_str().to_string())
        }
        fn read_queue(&self) -> Result<Queue, Trouble> {
            Ok(self.0.borrow().clone())
        }
        fn write_queue(&self, q: &Queue) -> Result<(), Trouble> {
            *self.0.borrow_mut() = q.clone();
            Ok(())
        }
        fn pile(&self) -> Result<Vec<Vec<u8>>, Trouble> {
            Ok(Vec::new())
        }
        fn behind(&self, missing: usize) -> Trouble {
            Trouble::text(Code::NewerElsewhere, missing.to_string())
        }
        fn unread(&self, f: Trouble) -> Trouble {
            f
        }
        fn mark(&self) -> Result<Option<zikaron_seal::unfetched::State>, Trouble> {
            Ok(None)
        }
        fn machine(&self) -> Result<std::path::PathBuf, Trouble> {
            Ok(std::path::PathBuf::new())
        }
    }

    /// Settling reads the queue anew and moves only entries still awaiting the batch heard about: included, those
    /// awaiting a set holding the included transaction leave (one another sender settled already, or one waiting
    /// on another transaction, stays as it is); void and reverted move only entries awaiting exactly these
    /// transactions; not yet writes nothing.
    #[test]
    fn settling_moves_only_what_still_awaits_its_batch() {
        let id = |n: u8| zikaron::hexfmt::encode(&[n; 32]);
        let tx = |n: u8| zikaron::hexfmt::encode(&[0xf0 | n; 32]);
        let heard = |receipt: Receipt, voided: bool, ids: Vec<String>| Heard {
            tx: tx(1),
            chain: 7,
            txs: vec![tx(1)],
            ids,
            confirmed: matches!(receipt, Receipt::Included { status: 1, .. }),
            receipt,
            gas: None,
            calldata: String::new(),
            stuck: None,
            voided,
            waited: 0,
        };
        let fresh = || {
            let mut q = Queue::default();
            for n in 1..=4 {
                let _ = q.push(&id(n), n as u64);
            }
            q.mark(&[id(1), id(2)], Step::Submitted { tx: tx(1), chain: 7, nonce: Some(0) });
            q.mark(&[id(3)], Step::Resent { txs: vec![tx(1), tx(2)], chain: 7, nonce: Some(0) });
            q.mark(&[id(4)], Step::Submitted { tx: tx(3), chain: 7, nonce: Some(1) });
            Mem(std::cell::RefCell::new(q))
        };
        let all = vec![id(1), id(2), id(3), id(4), id(5)];
        let m = fresh();
        let l = settle(&m, heard(Receipt::Included { status: 1, block: 9 }, false, all.clone())).expect("settled");
        assert_eq!(l.dropped, 3, "the two awaiting it and the one resent over it");
        let q = m.0.borrow();
        assert!(q.anchored_here(&id(1)) && q.anchored_here(&id(3)) && q.block_of(&id(2)).map(|b| b.block) == Some(9));
        assert_eq!(q.step_of(&id(4)), Some(&Step::Submitted { tx: tx(3), chain: 7, nonce: Some(1) }), "another batch stays");
        assert!(!q.anchored_here(&id(5)), "one not in the queue is not recorded");
        drop(q);
        let m = fresh();
        let l = settle(&m, heard(Receipt::Void, true, all.clone())).expect("settled");
        let q = m.0.borrow();
        assert_eq!((q.step_of(&id(1)), q.step_of(&id(2))), (Some(&Step::Queued), Some(&Step::Queued)), "void: back");
        assert!(q.step_of(&id(3)).is_some_and(Step::in_flight) && q.step_of(&id(4)).is_some_and(Step::in_flight), "others stay in flight");
        assert!(l.voided, "entries went back: answered as void");
        drop(q);
        // Void judged on transactions none of these entries awaits any more (another sender resent them, maybe
        // into a block): nothing moves, and it is not answered as void.
        let m = fresh();
        let l = settle(&m, heard(Receipt::Void, true, vec![id(3)])).expect("settled");
        assert!(m.0.borrow().step_of(&id(3)).is_some_and(Step::in_flight), "a resent batch stays as it is");
        assert!(!l.voided, "no entry went back: no void of theirs");
        // A second wait on the same batch, settling after the first moved its entries back: still void.
        let m = fresh();
        let first = settle(&m, heard(Receipt::Void, true, all.clone())).expect("settled");
        let second = settle(&m, heard(Receipt::Void, true, vec![id(1), id(2)])).expect("settled again");
        assert!(first.voided && second.voided, "both waits answer void");
        let m = fresh();
        settle(&m, heard(Receipt::Included { status: 0, block: 9 }, false, all.clone())).expect("settled");
        assert_eq!(m.0.borrow().step_of(&id(1)), Some(&Step::Reverted { tx: tx(1), chain: 7 }));
        assert!(m.0.borrow().step_of(&id(3)).is_some_and(Step::in_flight), "a resent batch awaits other transactions");
        let m = fresh();
        let before = m.0.borrow().to_bytes();
        settle(&m, heard(Receipt::NotYet, false, all)).expect("settled");
        assert_eq!(m.0.borrow().to_bytes(), before, "not yet: nothing written");
    }

    /// The balance gate's comparison at its edges: nothing needed and nothing held passes; exactly enough passes;
    /// one wei short is refused with both numbers; the largest amounts compare whole.
    #[test]
    fn the_balance_gate_compares_at_its_edges() {
        assert!(funds_check(0, 0).is_ok());
        assert!(funds_check(10, 10).is_ok());
        assert_eq!(funds_check(10, 9), Err(Trouble::new(Code::InsufficientFunds, Why::Funds { need: 10, have: 9 })));
        assert!(funds_check(u128::MAX, u128::MAX).is_ok());
        assert_eq!(funds_check(u128::MAX, u128::MAX - 1).map_err(|t| t.code), Err(Code::InsufficientFunds));
    }
}
