use crate::out::{self, Answer, Said};
use crate::sealed::Opened;
use crate::verbs;
use zikaron_send::batch::{self, Batch, GasAsk, Heard, Landed, Receipt, Sender, Stuck};
use zikaron_send::node::{Endpoint, NodeAddr};
use zikaron_send::{Code, Trouble, Why};

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

    fn behind(&self, lacks: &[String]) -> Fail {
        let state = zikaron_seal::unfetched::State::lacking(lacks);
        match self.0.place_mark(&state) {
            Ok(()) => Trouble::text(Code::NewerElsewhere, lacks.len().to_string()).into(),
            Err(t) => t.into(),
        }
    }

    fn unread(&self, f: Fail) -> Fail {
        f
    }

    fn mark(&self) -> Result<Option<zikaron_seal::unfetched::State>, Fail> {
        Ok(self.0.mark()?)
    }

    fn lift(&self) -> Result<(), Fail> {
        Ok(self.0.lift_mark()?)
    }

    fn machine(&self) -> Result<std::path::PathBuf, Fail> {
        Ok(self.0.machine().to_path_buf())
    }
}

impl zikaron_send::Facts for Home<'_> {
    fn known(&self) -> zikaron_anchor::scan::Known {
        zikaron_send::Facts::known(&self.0.checked())
    }

    fn known_single(&self) -> zikaron_anchor::scan::Known {
        zikaron_send::Facts::known_single(&self.0.checked())
    }

    fn add(&self, runs: &[(Vec<u64>, zikaron_anchor::scan::Sightings)], thin: &[u64]) {
        zikaron_send::Facts::add(&self.0.checked(), runs, thin)
    }
}

enum Gate {
    Fail(Fail),
    Tail,
}

impl From<Trouble> for Gate {
    fn from(t: Trouble) -> Gate {
        Gate::Fail(Fail::Send(t))
    }
}

impl From<zikaron_seal::Trouble> for Gate {
    fn from(t: zikaron_seal::Trouble) -> Gate {
        Gate::Fail(Fail::Seal(t))
    }
}

struct Writing<'a>(Home<'a>, bool);

impl zikaron_send::End for Writing<'_> {
    type Fail = Gate;

    fn code_of(f: &Gate) -> Option<String> {
        match f {
            Gate::Fail(f) => Home::code_of(f),
            Gate::Tail => None,
        }
    }

    fn read_queue(&self) -> Result<zikaron_seal::queue::Queue, Gate> {
        self.0.read_queue().map_err(Gate::Fail)
    }

    fn write_queue(&self, q: &zikaron_seal::queue::Queue) -> Result<(), Gate> {
        self.0.write_queue(q).map_err(Gate::Fail)
    }

    fn pile(&self) -> Result<Vec<Vec<u8>>, Gate> {
        self.0.pile().map_err(Gate::Fail)
    }

    fn behind(&self, lacks: &[String]) -> Gate {
        Gate::Fail(self.0.behind(lacks))
    }

    fn unread(&self, f: Gate) -> Gate {
        f
    }

    fn mark(&self) -> Result<Option<zikaron_seal::unfetched::State>, Gate> {
        self.0.mark().map_err(Gate::Fail)
    }

    fn lift(&self) -> Result<(), Gate> {
        self.0.lift().map_err(Gate::Fail)
    }

    fn machine(&self) -> Result<std::path::PathBuf, Gate> {
        self.0.machine().map_err(Gate::Fail)
    }

    fn tail(&self) -> Result<(), Gate> {
        if self.1 { Err(Gate::Tail) } else { Ok(()) }
    }
}

struct Looked<'a>(&'a crate::sealed::Reading);

impl zikaron_send::gate::Held for Looked<'_> {
    type Fail = Fail;

    fn pile(&self) -> Result<Vec<Vec<u8>>, Fail> {
        Ok(self.0.items.clone())
    }

    fn behind(&self, lacks: &[String]) -> Fail {
        let state = zikaron_seal::unfetched::State::lacking(lacks);
        match self.0.place_mark(&state) {
            Ok(()) => Trouble::text(Code::NewerElsewhere, lacks.len().to_string()).into(),
            Err(t) => t.into(),
        }
    }

    fn unread(&self, f: Fail) -> Fail {
        f
    }
}

impl zikaron_send::Facts for Looked<'_> {
    fn known(&self) -> zikaron_anchor::scan::Known {
        zikaron_send::Facts::known(&self.0.checked())
    }

    fn known_single(&self) -> zikaron_anchor::scan::Known {
        zikaron_send::Facts::known_single(&self.0.checked())
    }

    fn add(&self, runs: &[(Vec<u64>, zikaron_anchor::scan::Sightings)], thin: &[u64]) {
        zikaron_send::Facts::add(&self.0.checked(), runs, thin)
    }
}

pub fn export_gate(r: &crate::sealed::Reading) -> Result<zikaron_send::gate::Pass, Answer> {
    let at = r.home().display().to_string();
    let fail = |f: Fail| -> Answer {
        match f {
            Fail::Seal(t) => crate::sealed::refused(&t),
            Fail::Send(t) if matches!(t.code, Code::NotFetched | Code::NewerElsewhere | Code::Broken) => read_only(&t),
            Fail::Send(t) => chain_refused(&t, &at),
        }
    };
    let cli = r.machine_settings().map_err(|t| crate::sealed::refused(&t))?;
    zikaron_send::node::take_machine_route(cli.proxy.as_deref());
    let ground = r.ground().map_err(|t| crate::sealed::refused(&t))?;
    let ask = gate_ask(r.home(), &ground, r.own.clone()).map_err(fail)?;
    zikaron_send::gate::pass(&ask, &Looked(r)).map_err(fail)
}

pub(crate) fn notice_pass(home: &Opened) -> Result<zikaron_send::gate::Pass, Answer> {
    let run = || -> Result<zikaron_send::gate::Pass, Fail> {
        let cli = home.machine_settings()?;
        zikaron_send::node::take_machine_route(cli.proxy.as_deref());
        let ground = home.ground()?;
        let ask = gate_ask(home.home(), &ground, Some(home.author.clone()))?;
        zikaron_send::gate::pass(&ask, &Home(home))
    };
    run().map_err(refused)
}

pub(crate) fn chain_refused(t: &Trouble, at: &str) -> Answer {
    if crate::named::chain_unread_of(t) {
        return crate::named::of_trouble(zikaron_glue::door::Own::ChainUnread, t, at);
    }
    refused(Fail::Send(t.clone()))
}

fn read_only(t: &Trouble) -> Answer {
    read_only_named(t.code.as_str(), &said(&t.why))
}

pub fn read_only_named(code: &str, detail: &str) -> Answer {
    crate::named::of(zikaron_glue::door::Own::ReadOnly, code, detail, false, "")
}

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
    zikaron_send::node::take_machine_route(cli.proxy.as_deref());
    let ground = home.ground()?;
    let queue = home.read_queue()?;
    let queue = match ground.chain_id {
        Some(chain) if cli.cli_anchor == zikaron_seal::machine::CliAnchor::Send && batch::out_first(&queue).is_none() => {
            let at = Place::of(&ground, chain)?;
            let published: Vec<String> = home.last_anchored()?.map(|a| a.rows.into_iter().map(|(id, _)| id.to_ascii_lowercase()).collect()).unwrap_or_default();
            let facts = Home(home);
            zikaron_send::keep::caught_up(&Home(home), chain, &ground.other_chains(), now_secs(), &published, |pile| zikaron_send::keep::owed_on(&at.eps, chain, &at.registry, ground.from_block, pile, &facts))?
        }
        _ => queue,
    };
    let places = zikaron_seal::queue::Place::of_pile(&home.items);
    let place = |id: &str| places.get(&id.to_ascii_lowercase()).copied();
    let lane = queue.next_batch(usize::MAX, place);
    let next = lane.as_ref().map(|(_, ids)| ids.clone()).unwrap_or_default();
    let count = queue.sendable();
    match zikaron_seal::machine::turn(cli.cli_anchor, next.len()) {
        zikaron_seal::machine::Turn::Leave => Ok(verbs::queued(count as u64)),
        zikaron_seal::machine::Turn::Nothing => match batch::out_first(&queue) {
            Some((txs, chain, ids)) => {
                let nonce = ids.first().and_then(|id| queue.step_on(id, chain)).and_then(zikaron_seal::queue::Step::nonce);
                let at = Place::of(&ground, chain)?;
                let sender = Sender { from: who(home)?, registry: at.registry };
                let l = match landed(home, at.urls(None), chain, txs, ids.clone(), None, wait, Some(sender), nonce)? {
                    Ok(l) => l,
                    Err(out) => return Ok(out),
                };
                match batch::again_at(&l, cli.cli_anchor) {
                    Some((stuck, fees)) => again(home, &ground, &at, ids, stuck, fees, wait, &l),
                    None => Ok(answer_of(&l)),
                }
            }
            None => Err(Trouble::text(Code::QueueEmpty, "").into()),
        },
        zikaron_seal::machine::Turn::Send => {
            ground.chain_id.ok_or_else(|| Trouble::text(Code::NoChainId, ""))?;
            let chain = lane.as_ref().map(|(c, _)| *c).unwrap_or_default();
            let at = Place::of(&ground, chain)?;
            let ids = next;
            let hashes = zikaron_seal::queue::Queue::hashes(&ids).map_err(|t| Trouble::text(Code::ContentShape, t.tail))?;
            let signer = Key32(home.key);
            let from = who(home)?;
            let carried = batch::carried_of(&home.pile()?, &ids, chain);
            if carried.is_some() {
                if let Some(refused) = crate::faceverbs::sealed_here(home, &ground) {
                    return Ok(refused);
                }
            }
            let b = Batch { ids: ids.clone(), hashes, chain, registry: at.registry, urls: at.urls(None), backoff: zikaron_send::node::SEND_BACKOFF.to_vec(), basis: lanes_of(&ground), carried };
            let data = b.calldata();
            let call = zikaron_anchor::send::estimate_call(&from, &at.registry, &data);
            let est = batch::estimate(&GasAsk { chain, call, data, eps: at.eps.clone() })?;
            let ask = gate_ask(home.home(), &ground, Some(home.author.clone()))?;
            let sent = match batch::anchor(&Home(home), b, &signer, Some(est.gas), est.fees, ask.clone()) {
                Ok(s) => s,
                Err(f) => match batch::out_on(&home.read_queue()?, &ids, chain) {
                    Some((txs, _, _)) => return Ok(verbs::tx_unheard(txs.last().map(String::as_str).unwrap_or_default(), &fail_said(&f))),
                    None => {
                        if matches!(&f, Fail::Send(t) if t.code == Code::OnChain) {
                            seen_on_chain(home, &ask, chain);
                        }
                        return Err(f);
                    }
                },
            };
            let txs = sent.ids.first().and_then(|id| sent.queue.step_on(id, chain)).and_then(zikaron_seal::queue::Step::awaited).map(|(t, _)| t).unwrap_or_else(|| vec![sent.tx.clone()]);
            let nonce = sent.ids.first().and_then(|id| sent.queue.step_of(id)).and_then(zikaron_seal::queue::Step::nonce);
            let sender = Sender { from, registry: at.registry };
            settled(home, at.urls(Some(&sent.url)), sent.chain, txs, sent.ids, sent.gas, wait, Some(sender), nonce)
        }
    }
}

fn gate_ask(home: &std::path::Path, ground: &zikaron_seal::ground::Ground, own: Option<String>) -> Result<zikaron_send::gate::Ask, Fail> {
    let eps = zikaron_send::node::settings_nodes(ground);
    Ok(zikaron_send::gate::ask(home.to_path_buf(), ground, &eps, own)?)
}

#[allow(clippy::too_many_arguments)]
fn again(home: &mut Opened, ground: &zikaron_seal::ground::Ground, at: &Place, ids: Vec<String>, stuck: Stuck, fees: zikaron_anchor::send::Fees, wait: std::time::Duration, l: &Landed) -> Result<Answer, Fail> {
    let before = stuck.txs.clone();
    let stuck_chain = stuck.chain;
    if batch::out_for(&home.read_queue()?, &ids).map(|(t, _, _)| t).as_ref() != Some(&before) {
        return Ok(answer_of(l));
    }
    let hashes = zikaron_seal::queue::Queue::hashes(&ids).map_err(|t| Trouble::text(Code::ContentShape, t.tail))?;
    let ask = gate_ask(home.home(), ground, Some(home.author.clone()))?;
    let carried = batch::carried_of(&home.pile()?, &ids, stuck.chain);
    let b = Batch { ids: ids.clone(), hashes, chain: stuck.chain, registry: at.registry, urls: at.urls(None), backoff: zikaron_send::node::SEND_BACKOFF.to_vec(), basis: lanes_of(ground), carried };
    let signer = Key32(home.key);
    let sender = Sender { from: stuck.from, registry: at.registry };
    let nonce = stuck.nonce;
    let sent = match batch::resend(&Home(home), b, stuck, fees, &signer, ask) {
        Ok(s) => s,
        Err(f) => match batch::out_on(&home.read_queue()?, &ids, stuck_chain).map(|(txs, _, _)| txs) {
            Some(txs) if txs != before => return Ok(verbs::tx_unheard(txs.last().map(String::as_str).unwrap_or_default(), &fail_said(&f))),
            _ => return Err(f),
        },
    };
    let txs = sent.ids.first().and_then(|id| sent.queue.step_on(id, sent.chain)).and_then(zikaron_seal::queue::Step::awaited).map(|(t, _)| t).unwrap_or_else(|| vec![sent.tx.clone()]);
    settled(home, at.urls(Some(&sent.url)), sent.chain, txs, sent.ids, None, wait, Some(sender), Some(nonce))
}

fn seen_on_chain(home: &Opened, ask: &zikaron_send::gate::Ask, chain: u64) {
    let Ok(r) = zikaron_send::gate::read(ask, &Home(home)) else { return };
    let Ok(q) = home.read_queue() else { return };
    let here: Vec<String> = r.on_chain.iter().filter(|(c, _)| *c == chain).map(|(_, h)| h.clone()).collect();
    let on_chain = leaving(&q, &here, chain);
    if !on_chain.is_empty() {
        let _ = zikaron_send::keep::follow(&Home(home), &on_chain, &[], &[], now_secs());
    }
}

pub fn leaving(q: &zikaron_seal::queue::Queue, anchored: &[String], chain: u64) -> Vec<(String, u64)> {
    let bare = |s: &str| s.trim().trim_start_matches("0x").to_ascii_lowercase();
    q.items.iter().filter(|x| x.chain == chain && !x.step.in_flight() && anchored.contains(&bare(&x.id))).map(|x| (x.id.clone(), chain)).collect()
}

fn fail_said(f: &Fail) -> String {
    match f {
        Fail::Send(t) => said(&t.why),
        Fail::Seal(t) => t.tail.clone(),
    }
}

#[allow(clippy::too_many_arguments)]
fn settled(home: &mut Opened, urls: Vec<NodeAddr>, chain: u64, txs: Vec<String>, ids: Vec<String>, gas: Option<u64>, wait: std::time::Duration, sender: Option<Sender>, nonce: Option<u64>) -> Result<Answer, Fail> {
    Ok(match landed(home, urls, chain, txs, ids, gas, wait, sender, nonce)? {
        Ok(l) => answer_of(&l),
        Err(out) => out,
    })
}

#[allow(clippy::too_many_arguments)]
fn landed(home: &mut Opened, urls: Vec<NodeAddr>, chain: u64, txs: Vec<String>, ids: Vec<String>, gas: Option<u64>, wait: std::time::Duration, sender: Option<Sender>, nonce: Option<u64>) -> Result<Result<Landed, Answer>, Fail> {
    let carried = batch::carried_of(&home.pile()?, &ids, chain);
    home.let_go();
    let last = txs.last().cloned().unwrap_or_default();
    let mut heard: Heard = match batch::wait(urls, chain, txs, ids, gas, wait, &zikaron_anchor::send::RECEIPT_BACKOFF, sender, nonce, carried.as_deref()) {
        Ok(h) => h,
        Err(t) => return Ok(Err(verbs::tx_unheard(&last, &said(&t.why)))),
    };
    batch::landed_facts(&Home(home), &mut heard);
    home.take_back()?;
    Ok(Ok(batch::settle(&Home(home), heard)?))
}

fn answer_of(l: &Landed) -> Answer {
    match &l.receipt {
        _ if l.voided => verbs::tx_void(&l.tx),
        Receipt::Included { status: 1, block } => verbs::anchored(&l.tx, Some(*block)),
        Receipt::Included { status, .. } => verbs::tx_failed(&l.tx, *status),
        Receipt::Unheard(said) => verbs::tx_unheard(&l.tx, said),
        Receipt::NotYet | Receipt::Split { .. } | Receipt::Void | Receipt::Unused => verbs::tx_not_yet(&l.tx, l.waited),
    }
}

pub(crate) fn nodes_of(g: &zikaron_seal::ground::Ground, chain: u64) -> Vec<Endpoint> {
    zikaron_send::node::nodes_of(g, chain)
}

pub(crate) fn proofs_beside(r: &crate::sealed::Reading, package: &std::path::Path) {
    let Ok(ground) = r.ground() else { return };
    let Some(root) = zikaron_glue::book::root_of(&r.items).ok().and_then(|x| zikaron::hexfmt::decode(&x)).and_then(|b| <[u8; 20]>::try_from(b).ok()) else { return };
    let senders = zikaron_send::proofs::senders_from(&zikaron_glue::basis::senders_of(&r.items));
    let proofs = zikaron_send::proofs::for_export(&r.checked().0, &senders, &ground, &zikaron_send::node::endpoint_at);
    let _ = zikaron_glue::anchorproofs::write_beside(package, &root, &proofs);
}

struct Place {
    registry: [u8; 20],
    eps: Vec<Endpoint>,
}

impl Place {
    fn of(g: &zikaron_seal::ground::Ground, chain: u64) -> Result<Place, Fail> {
        let (registry, _) = g.window_of(chain).ok_or_else(|| Trouble::text(Code::NoRegistry, ""))?;
        let eps = nodes_of(g, chain);
        if eps.is_empty() {
            return Err(Trouble::text(Code::NoEndpoint, chain.to_string()).into());
        }
        Ok(Place { registry, eps })
    }

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

fn lanes_of(g: &zikaron_seal::ground::Ground) -> Vec<zikaron_send::batch::Lane> {
    g.chain_id
        .into_iter()
        .chain(g.other_chains())
        .filter_map(|c| {
            let (registry, from_block) = g.window_of(c)?;
            Some(zikaron_send::batch::Lane { chain: c, registry, urls: nodes_of(g, c).into_iter().map(|e| e.url).collect(), from_block })
        })
        .collect()
}

fn who(home: &Opened) -> Result<[u8; 20], Fail> {
    zikaron::cryptox::address_of_privkey(&home.key).ok_or_else(|| Trouble::new(Code::KeyMalformed, Why::NoAddress).into())
}

pub fn write_gates(home: &Opened, tail_refused: bool) -> Option<Answer> {
    match zikaron_send::shut::check(&Writing(Home(home), tail_refused)) {
        Ok(()) | Err(Gate::Tail) => None,
        Err(Gate::Fail(f)) => gate_answer(Err(f)),
    }
}

pub fn open_to_write<T>(at: &str, now: u64, tail: impl FnOnce(&Opened) -> Result<T, Answer>) -> Result<(Opened, T), Answer> {
    let home = crate::sealed::open(at, now).map_err(|t| crate::sealed::refused(&t))?;
    let read = tail(&home);
    if let Some(refused) = write_gates(&home, read.is_err()) {
        return Err(refused);
    }
    read.map(|t| (home, t))
}

pub fn hand_gates(home: &Opened) -> Option<Answer> {
    match zikaron_send::shut::check(&Home(home)) {
        Err(Fail::Send(t)) if t.code == Code::HandedOff => None,
        judged => gate_answer(judged),
    }
}

fn gate_answer(judged: Result<(), Fail>) -> Option<Answer> {
    match judged {
        Ok(()) => None,
        Err(Fail::Seal(t)) => Some(crate::sealed::refused(&t)),
        Err(Fail::Send(t)) if t.code == Code::HandedOff => Some(crate::sealed::refused(&zikaron_seal::Trouble::new(zikaron_seal::Code::HandedOff, said(&t.why)))),
        Err(Fail::Send(t)) if matches!(t.code, Code::NotFetched | Code::NewerElsewhere | Code::Broken) => Some(read_only(&t)),
        Err(f) => Some(refused(f)),
    }
}

fn refused(f: Fail) -> Answer {
    match f {
        Fail::Seal(t) => crate::sealed::refused(&t),
        Fail::Send(Trouble { code: Code::OnChain, why: Why::OnChain { names, tx, block }, .. }) => verbs::on_chain(&names, &tx, block),
        Fail::Send(Trouble { code: Code::TooEarly, why: Why::TooEarly { chain, head_time, after }, .. }) => verbs::too_early(&verbs::fence_detail(chain, head_time, after)),
        Fail::Send(t @ Trouble { why: Why::Scan { .. }, .. }) if matches!(&t.why, Why::Scan { refused, .. } if refused.iter().any(|r| r.over_asks().is_some())) => send_refused(t.code.as_str(), &said(&t.why), t.network(), out::over_asks_line()),
        Fail::Send(t) => send_refused(t.code.as_str(), &said(&t.why), t.network(), Said::SendRefused.text().to_string()),
    }
}

pub fn send_refused(code: &str, detail: &str, network: bool, line: String) -> Answer {
    crate::named::send_refused(code, detail, network, line)
}

pub use zikaron_send::said;

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
