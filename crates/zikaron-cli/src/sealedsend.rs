//! `anchor --ledger <a home's ledger>`: the command line sends a home's queue with its credential, through the
//! one sender the desktop sends by (`zikaron_send::batch`).
//!
//! What it sends and how come from the home and this machine, never from flags: the chain, the registry and the
//! nodes from the home's settings (`zikaron_seal::ground`), the proxy and the switch "the command line anchors"
//! from this machine's sealed settings (`zikaron_seal::machine`), the key from the vault through the credential,
//! as for a write (`sealed::open`). The switch is judged once, as the desktop's door judges it
//! (`zikaron_seal::machine::turn`): send the entries that can be sent; leave them queued (`E_QUEUED`, exit 3); with
//! none to send, ask about a batch in flight (a sender stopped half way), or say the queue is empty.
//!
//! A send goes in the sender's four stages. The first two (ready, sign and send) hold the home's writer lock, taken
//! at the opening; the receipt wait holds none, so the desktop may write and send the rest of the queue meanwhile;
//! the lock is taken back to settle, on the queue read anew. A fee-bump resend is the desktop's alone.
//!
//! Two things the desktop's own rules do on its side are done on this road too, by the same rules. A batch in
//! flight that no node holds and whose nonce is still unused is signed again at that very nonce and broadcast when
//! the switch says send ([`again`]; the direct `anchor` reruns by the same rule, `zikaron_anchor::send::unheld`), so
//! at most one transaction at that nonce is ever included. And entries the chain already holds leave the queue by
//! the self-audit's one judgement, the reading every node agrees on, never one node's word ([`seen_on_chain`]).
//!
//! The answers are the door's (`desk`), built by the same functions: anchored, reverted, not yet, out of sight,
//! void; already on chain `E_ON_CHAIN`; any other refusal before anything went out `E_SEND_REFUSED`, with the
//! sender's code spelled as the desktop spells it and its evidence in this command line's words ([`said`]).

use crate::codes::{Key, Reason};
use crate::out::{self, s, Answer, Said};
use crate::sealed::Opened;
use crate::verbs;
use zikaron_seal::machine::CliAnchor;
use zikaron_send::batch::{self, Batch, GasAsk, Heard, Landed, Offer, Receipt, Sender, Stuck};
use zikaron_send::node::{Endpoint, NodeAddr};
use zikaron_send::{Code, Trouble, Why};

/// A refusal on this road: the sender's, or the sealed layer's (the credential, the lock, a sealed file).
pub enum Fail {
    Send(Trouble),
    Seal(zikaron_seal::Trouble),
}

impl From<Trouble> for Fail {
    fn from(t: Trouble) -> Fail {
        Fail::Send(t)
    }
}

impl From<zikaron_seal::Trouble> for Fail {
    fn from(t: zikaron_seal::Trouble) -> Fail {
        Fail::Seal(t)
    }
}

/// The home opened with the credential, as the sender sees it.
struct Home<'a>(&'a Opened);

impl zikaron_send::End for Home<'_> {
    type Fail = Fail;

    fn code_of(f: &Fail) -> Option<String> {
        Some(match f {
            Fail::Send(t) => t.code.as_str().to_string(),
            Fail::Seal(t) => crate::sealed::word(t.code),
        })
    }

    fn read_queue(&self) -> Result<zikaron_seal::queue::Queue, Fail> {
        Ok(self.0.read_queue()?)
    }

    fn write_queue(&self, q: &zikaron_seal::queue::Queue) -> Result<(), Fail> {
        Ok(self.0.write_queue(q)?)
    }

    fn pile(&self) -> Result<Vec<Vec<u8>>, Fail> {
        Ok(self.0.pile()?)
    }

    /// The home takes the "newer entries elsewhere" mark as the desktop places it; a mark that cannot be written is
    /// the refusal instead.
    fn behind(&self, missing: usize) -> Fail {
        match self.0.place_mark(zikaron_seal::unfetched::State::NewerElsewhere { missing }) {
            Ok(()) => Trouble::text(Code::NewerElsewhere, missing.to_string()).into(),
            Err(t) => t.into(),
        }
    }

    fn unread(&self, f: Fail) -> Fail {
        f
    }

    fn mark(&self) -> Result<Option<zikaron_seal::unfetched::State>, Fail> {
        Ok(self.0.mark()?)
    }

    fn machine(&self) -> Result<std::path::PathBuf, Fail> {
        Ok(self.0.machine().to_path_buf())
    }
}

/// No record of checked chain facts on this road: every fact is asked about.
impl zikaron_send::Facts for Home<'_> {
    fn known(&self) -> zikaron_anchor::scan::Known {
        zikaron_send::Facts::known(&zikaron_send::Bare)
    }

    fn add(&self, runs: &[(Vec<u64>, zikaron_anchor::scan::Sightings)], thin: &[u64]) {
        zikaron_send::Facts::add(&zikaron_send::Bare, runs, thin)
    }
}

/// The home's key, lent to the sender for one signature at a time.
struct Key32([u8; 32]);

impl batch::Signer for Key32 {
    fn from(&self) -> Option<[u8; 20]> {
        zikaron::cryptox::address_of_privkey(&self.0)
    }

    fn sign(&self, chain: u64, registry: Option<[u8; 20]>, hashes: &[[u8; 32]], input: Option<Vec<u8>>, fees: zikaron_anchor::send::Fees, nonce: u64) -> Result<(Vec<u8>, [u8; 32]), zikaron_anchor::rpc::Trouble> {
        zikaron_anchor::send::sign_at(&self.0, chain, zikaron_anchor::send::Form::Registry, registry, hashes, input, fees, nonce)
    }
}

impl Drop for Key32 {
    fn drop(&mut self) {
        zikaron_os::wipe(&mut self.0);
    }
}

/// `anchor --ledger <ledger>` for a home the desktop keeps, waiting at most `wait` for a receipt.
pub fn anchor(ledger: &str, wait: std::time::Duration) -> Answer {
    crate::seam();
    let mut home = match crate::sealed::open(ledger, now_secs()) {
        Ok(h) => h,
        Err(t) => return crate::sealed::refused(&t),
    };
    match run(&mut home, wait) {
        Ok(a) => a,
        Err(f) => refused(f),
    }
}

fn run(home: &mut Opened, wait: std::time::Duration) -> Result<Answer, Fail> {
    let cli = home.machine_settings()?;
    zikaron_anchor::rpc::set_choice(zikaron_send::node::route_of(cli.proxy.as_deref()));
    let ground = home.ground()?;
    let queue = home.read_queue()?;
    let count = queue.sendable();
    match zikaron_seal::machine::turn(cli.cli_anchor, count) {
        zikaron_seal::machine::Turn::Leave => Ok(verbs::queued(count as u64)),
        zikaron_seal::machine::Turn::Nothing => match queue.submitted().into_iter().next() {
            Some((txs, chain, ids)) => {
                let nonce = ids.first().and_then(|id| queue.step_of(id)).and_then(zikaron_seal::queue::Step::nonce);
                let at = Place::of(&ground, chain)?;
                let sender = Sender { from: who(home)?, registry: at.registry };
                let l = match landed(home, at.urls(None), chain, txs, ids.clone(), None, wait, Some(sender), nonce)? {
                    Ok(l) => l,
                    Err(out) => return Ok(out),
                };
                match again_at(&l, cli.cli_anchor) {
                    Some((stuck, fees)) => again(home, &ground, &at, ids, stuck, fees, wait, &l),
                    None => Ok(answer_of(&l)),
                }
            }
            None => Err(Trouble::text(Code::QueueEmpty, "").into()),
        },
        zikaron_seal::machine::Turn::Send => {
            let chain = ground.chain_id.ok_or_else(|| Trouble::text(Code::NoChainId, ""))?;
            let at = Place::of(&ground, chain)?;
            let ids = queue.take_ids(count);
            let hashes = zikaron_seal::queue::Queue::hashes(&ids).map_err(|t| Trouble::text(Code::ContentShape, t.tail))?;
            let signer = Key32(home.key);
            let from = who(home)?;
            let data = zikaron_anchor::send::registry_calldata(&hashes);
            let call = zikaron_anchor::send::estimate_call(&from, &at.registry, &data);
            let est = batch::estimate(&GasAsk { chain, call, data, eps: at.eps.clone() })?;
            let ask = gate_ask(home, &ground, &at, chain)?;
            let b = Batch { ids: ids.clone(), hashes, chain, registry: at.registry, urls: at.urls(None), backoff: zikaron_send::node::SEND_BACKOFF.to_vec() };
            let sent = match batch::anchor(&Home(home), b, &signer, Some(est.gas), est.fees, ask.clone(), ground.from_block) {
                Ok(s) => s,
                // Failed after the batch was landed in flight (no node answered the broadcast; its bytes may be in a
                // pool): it is out, not refused, answered as a receipt no node gave for its transaction; the next run
                // asks about it by hash.
                Err(f) => match batch::out_for(&home.read_queue()?, &ids) {
                    Some((txs, _, _)) => return Ok(verbs::tx_unheard(txs.last().map(String::as_str).unwrap_or_default(), &fail_said(&f))),
                    None => {
                        // Nothing sent this time; what the chain holds already leaves the queue by the agreed
                        // reading, so the next run sends the rest.
                        if matches!(&f, Fail::Send(t) if t.code == Code::OnChain) {
                            seen_on_chain(home, &ask);
                        }
                        return Err(f);
                    }
                },
            };
            let txs = sent.ids.first().and_then(|id| sent.queue.step_of(id)).and_then(zikaron_seal::queue::Step::awaited).map(|(t, _)| t).unwrap_or_else(|| vec![sent.tx.clone()]);
            let nonce = sent.ids.first().and_then(|id| sent.queue.step_of(id)).and_then(zikaron_seal::queue::Step::nonce);
            let sender = Sender { from, registry: at.registry };
            settled(home, at.urls(Some(&sent.url)), sent.chain, txs, sent.ids, sent.gas, wait, Some(sender), nonce)
        }
    }
}

/// What the exit gate asks for this home's chain: its registry, its nodes, the scan window, the identity's own key.
fn gate_ask(home: &Opened, ground: &zikaron_seal::ground::Ground, at: &Place, chain: u64) -> Result<zikaron_send::gate::Ask, Fail> {
    Ok(zikaron_send::gate::ask(home.home().to_path_buf(), Some(chain), Some(at.registry), ground.from_block, &at.eps, Some(home.author.clone()))?)
}

/// Whether a settled wait leaves its batch to be signed again on this road, and at what fees: held by no node with
/// its nonce unused (offered again as recorded, `Offer::Unheld`), and the switch says send. Under "queue only"
/// nothing is sent; once resent the most times (`Offer::Spent`), or with the price unread, nothing is offered.
pub fn again_at(l: &Landed, switch: CliAnchor) -> Option<(Stuck, zikaron_anchor::send::Fees)> {
    match (&l.stuck, switch) {
        (Some(stuck @ Stuck { offer: Offer::Unheld { fees, .. }, .. }), CliAnchor::Send) if l.receipt == Receipt::Unused && !l.voided => Some((stuck.clone(), *fees)),
        _ => None,
    }
}

/// A batch no node holds whose nonce is unused, signed again as recorded (its entries, at that nonce, at the price
/// offered now) and broadcast by the sender's resend (`batch::resend`: the chain asked whether it holds these
/// anchors, the balance, the exit gate, the same key; the new hash recorded with the batch before its bytes go),
/// then waited for and settled as any send. Only the batch as it stands here: if another sender moved it while
/// this one waited (resent it, or settled it), nothing is signed and the wait's own answer stands.
#[allow(clippy::too_many_arguments)]
fn again(home: &mut Opened, ground: &zikaron_seal::ground::Ground, at: &Place, ids: Vec<String>, stuck: Stuck, fees: zikaron_anchor::send::Fees, wait: std::time::Duration, l: &Landed) -> Result<Answer, Fail> {
    let before = stuck.txs.clone();
    if batch::out_for(&home.read_queue()?, &ids).map(|(t, _, _)| t).as_ref() != Some(&before) {
        return Ok(answer_of(l));
    }
    let hashes = zikaron_seal::queue::Queue::hashes(&ids).map_err(|t| Trouble::text(Code::ContentShape, t.tail))?;
    let ask = gate_ask(home, ground, at, stuck.chain)?;
    let b = Batch { ids: ids.clone(), hashes, chain: stuck.chain, registry: at.registry, urls: at.urls(None), backoff: zikaron_send::node::SEND_BACKOFF.to_vec() };
    let signer = Key32(home.key);
    let sender = Sender { from: stuck.from, registry: at.registry };
    let nonce = stuck.nonce;
    let sent = match batch::resend(&Home(home), b, stuck, fees, &signer, ask, ground.from_block) {
        Ok(s) => s,
        // A new transaction kept as awaited (no node answered its broadcast): out, answered with it.
        Err(f) => match batch::out_for(&home.read_queue()?, &ids).map(|(txs, _, _)| txs) {
            Some(txs) if txs != before => return Ok(verbs::tx_unheard(txs.last().map(String::as_str).unwrap_or_default(), &fail_said(&f))),
            _ => return Err(f),
        },
    };
    let txs = sent.ids.first().and_then(|id| sent.queue.step_of(id)).and_then(zikaron_seal::queue::Step::awaited).map(|(t, _)| t).unwrap_or_else(|| vec![sent.tx.clone()]);
    settled(home, at.urls(Some(&sent.url)), sent.chain, txs, sent.ids, None, wait, Some(sender), Some(nonce))
}

/// Entries the chain already holds leave the queue, recorded as anchored, as the desktop's self-audit takes them
/// out (`keep::follow`): by the reading every node of the chain agrees on (the exit gate's, `gate::read`), only
/// entries none of whose transactions is out. A reading that does not agree, or does not come, moves nothing; the
/// next run asks again.
fn seen_on_chain(home: &Opened, ask: &zikaron_send::gate::Ask) {
    let Ok(r) = zikaron_send::gate::read(ask, &Home(home)) else { return };
    let Ok(q) = home.read_queue() else { return };
    let on_chain = leaving(&q, &r.anchored);
    if !on_chain.is_empty() {
        let _ = zikaron_send::keep::follow(&Home(home), &on_chain, &[], now_secs());
    }
}

/// The queued entries an agreed reading finds on chain (`anchored`, ids as the gate reads them: bare lower hex):
/// those none of whose transactions is out, as the queue spells them.
pub fn leaving(q: &zikaron_seal::queue::Queue, anchored: &[String]) -> Vec<String> {
    let bare = |s: &str| s.trim().trim_start_matches("0x").to_ascii_lowercase();
    q.items.iter().filter(|x| !x.step.in_flight() && anchored.contains(&bare(&x.id))).map(|x| x.id.clone()).collect()
}

/// A refusal's evidence in this command line's words.
fn fail_said(f: &Fail) -> String {
    match f {
        Fail::Send(t) => said(&t.why),
        Fail::Seal(t) => t.tail.clone(),
    }
}

/// The receipt wait and its settling, answered.
#[allow(clippy::too_many_arguments)]
fn settled(home: &mut Opened, urls: Vec<NodeAddr>, chain: u64, txs: Vec<String>, ids: Vec<String>, gas: Option<u64>, wait: std::time::Duration, sender: Option<Sender>, nonce: Option<u64>) -> Result<Answer, Fail> {
    Ok(match landed(home, urls, chain, txs, ids, gas, wait, sender, nonce)? {
        Ok(l) => answer_of(&l),
        Err(out) => out,
    })
}

/// The receipt wait and its settling: the lock let go while waiting, taken back to settle. A wait that fails is
/// answered at once (the inner `Err`): this batch is out, so it says so with its transaction, never as nothing sent.
#[allow(clippy::too_many_arguments)]
fn landed(home: &mut Opened, urls: Vec<NodeAddr>, chain: u64, txs: Vec<String>, ids: Vec<String>, gas: Option<u64>, wait: std::time::Duration, sender: Option<Sender>, nonce: Option<u64>) -> Result<Result<Landed, Answer>, Fail> {
    home.let_go();
    let last = txs.last().cloned().unwrap_or_default();
    let heard: Heard = match batch::wait(urls, chain, txs, ids, gas, wait, &zikaron_anchor::send::RECEIPT_BACKOFF, sender, nonce) {
        Ok(h) => h,
        Err(t) => return Ok(Err(verbs::tx_unheard(&last, &said(&t.why)))),
    };
    home.take_back()?;
    Ok(Ok(batch::settle(&Home(home), heard)?))
}

/// A settled wait as this road answers it, by the door's functions.
fn answer_of(l: &Landed) -> Answer {
    match &l.receipt {
        _ if l.voided => verbs::tx_void(&l.tx),
        Receipt::Included { status: 1, block } => verbs::anchored(&l.tx, Some(*block)),
        Receipt::Included { status, .. } => verbs::tx_failed(&l.tx, *status),
        Receipt::Unheard(said) => verbs::tx_unheard(&l.tx, said),
        Receipt::NotYet | Receipt::Split { .. } | Receipt::Void | Receipt::Unused => verbs::tx_not_yet(&l.tx, l.waited),
    }
}

/// The home's chain as its settings place it: the registry and that chain's nodes, in settings order.
struct Place {
    registry: [u8; 20],
    eps: Vec<Endpoint>,
}

impl Place {
    fn of(g: &zikaron_seal::ground::Ground, chain: u64) -> Result<Place, Fail> {
        let registry = g.registry.ok_or_else(|| Trouble::text(Code::NoRegistry, ""))?;
        let eps: Vec<Endpoint> = g.endpoints.iter().filter_map(|spec| Endpoint::parse(spec)).filter(|e| e.chain == chain).collect();
        if eps.is_empty() {
            return Err(Trouble::text(Code::NoEndpoint, chain.to_string()).into());
        }
        Ok(Place { registry, eps })
    }

    /// The nodes to ask, the one that took the transaction first.
    fn urls(&self, taken: Option<&NodeAddr>) -> Vec<NodeAddr> {
        let mut out: Vec<NodeAddr> = taken.cloned().into_iter().collect();
        for e in &self.eps {
            if !out.contains(&e.url) {
                out.push(e.url.clone());
            }
        }
        out
    }
}

/// The address the home's key sends from.
fn who(home: &Opened) -> Result<[u8; 20], Fail> {
    zikaron::cryptox::address_of_privkey(&home.key).ok_or_else(|| Trouble::new(Code::KeyMalformed, Why::NoAddress).into())
}

/// The home's write gates for an entry written with the credential, judged as for a send (`shut::check`, the
/// sender's one judgement): `None` when it may be written; else the answer, nothing written. A ledger handed off is
/// answered as every write answers it (`E_HANDED_OFF`, the handoff named); a home under its read-only mark, or whose
/// ledger's own entries break its chain, is `E_READ_ONLY` (`token` the code as the desktop spells it, `detail` its
/// evidence).
pub fn write_gates(home: &Opened) -> Option<Answer> {
    gate_answer(zikaron_send::shut::check(&Home(home)))
}

/// The same gates before the home's ledger is handed over with the credential (`hand-over`): a home under its
/// read-only mark, or whose ledger's own entries break its chain, is not handed (`E_READ_ONLY`, as a write is
/// answered); a ledger already handed off is handed again as before, the same handoff (`handoff::hand_over`).
pub fn hand_gates(home: &Opened) -> Option<Answer> {
    match zikaron_send::shut::check(&Home(home)) {
        Err(Fail::Send(t)) if t.code == Code::HandedOff => None,
        judged => gate_answer(judged),
    }
}

/// A gate's judgement as the credential's writes answer it: the three read-only gates by `E_READ_ONLY` with their
/// code, a ledger handed off as every write answers it, the sealed layer's own troubles (a list that does not read)
/// by their own reasons.
fn gate_answer(judged: Result<(), Fail>) -> Option<Answer> {
    match judged {
        Ok(()) => None,
        Err(Fail::Seal(t)) => Some(crate::sealed::refused(&t)),
        Err(Fail::Send(t)) if t.code == Code::HandedOff => Some(crate::sealed::refused(&zikaron_seal::Trouble::new(zikaron_seal::Code::HandedOff, said(&t.why)))),
        Err(Fail::Send(t)) if matches!(t.code, Code::NotFetched | Code::NewerElsewhere | Code::Broken) => {
            Some(out::denied(Reason::ReadOnly, vec![(Key::Token, s(t.code.as_str())), (Key::Detail, s(&said(&t.why)))]).telling(Said::ReadOnly.text().to_string()))
        }
        Err(f) => Some(refused(f)),
    }
}

/// A refusal as this road answers it: already on chain by its own reason with its evidence as values, the sealed
/// layer's by its own reasons, any other of the sender's `E_SEND_REFUSED` (exit 4 for the network's absence of an
/// answer, 1 otherwise).
fn refused(f: Fail) -> Answer {
    match f {
        Fail::Seal(t) => crate::sealed::refused(&t),
        Fail::Send(Trouble { code: Code::OnChain, why: Why::OnChain { names, tx, block }, .. }) => verbs::on_chain(&names, &tx, block),
        Fail::Send(t) => send_refused(t.code.as_str(), &said(&t.why), t.network(), Said::SendRefused.text().to_string()),
    }
}

/// `E_SEND_REFUSED`: nothing went out; `token` the code, `detail` its evidence; `network`, no answer (exit 4)
/// rather than a negative one (exit 1); `line` the line for people. The door's anchor refusals answer by it too.
pub fn send_refused(code: &str, detail: &str, network: bool, line: String) -> Answer {
    let members = vec![(Key::Token, s(code)), (Key::Detail, s(detail))];
    let a = if network { out::unanswered(Reason::SendRefused, members) } else { out::denied(Reason::SendRefused, members) };
    a.telling(line)
}

/// The sender's evidence in this command line's words: what was asked and what came back, plainly, in the
/// transport's own terms (nodes in their safe-to-say form, numbers in wei).
pub fn said(w: &Why) -> String {
    match w {
        Why::Text(t) => t.clone(),
        Why::Each(all) => all.iter().map(said).collect::<Vec<_>>().join("; "),
        Why::Node { node, trouble } => format!("{node}: {trouble:?}"),
        Why::Unreadable { node } => format!("{node}: answer unreadable"),
        Why::Scheme { node } => format!("{node}: not http or https"),
        Why::Port { node } => format!("{node}: port not 0-65535"),
        Why::NotAddress { node } => format!("{node}: not an address"),
        Why::NotSpec { node } => format!("{node}: not <chain>=<url>"),
        Why::AtChain { chain, why } => format!("{chain}={}", said(why)),
        Why::NoHeadNode { chain } => format!("{chain}: no node for the head"),
        Why::NoTimeNode { chain } => format!("{chain}: no node for the time"),
        Why::NotHeight { node, text } => format!("{node}: height {text}"),
        Why::Behind { node, lag, high } => format!("{node}: {lag} blocks behind {high}"),
        Why::NoTime { node } => format!("{node}: block without time or number"),
        Why::NotYetAt { has, not_yet } => format!("at {has}; not yet at {not_yet}"),
        Why::Differ { count, sources } => format!("{count} differ: {sources}"),
        Why::NoHistory { nodes } => format!("no records at {nodes}"),
        Why::WrongChainNode { node, serves, chain } => format!("{node}: chain {serves}, not {chain}"),
        Why::BalanceNotText(x) | Why::BalanceNotQty(x) => format!("eth_getBalance: {x}"),
        Why::GasNotText(x) | Why::GasNotQty(x) => format!("eth_estimateGas: {x}"),
        Why::NoAddress => "no address".to_string(),
        Why::Funds { need, have } => format!("need {need} wei, have {have} wei"),
        Why::OnChain { names, tx, block } => format!("{} {tx} {block}", names.join(" ")),
        Why::NoNodes => "no nodes".to_string(),
        Why::NonceUnread { node } => format!("{node}: nonce unreadable"),
        Why::Scan { unanswered, refused } => {
            let mut parts: Vec<String> = unanswered.iter().map(said).collect();
            parts.extend(refused.iter().map(zikaron_send::agree::scan_tail));
            parts.join("; ")
        }
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
