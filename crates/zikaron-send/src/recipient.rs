//! The public key behind an address, found on chain for a record package addressed to it: every network on offer
//! (the home's own and each read-only one), every node of each, asked at once, each by the one single-node reader
//! (`zikaron_anchor::scan::author_key`: the address's anchor logs through the registry, their transactions, the
//! key recovered from a signature the address itself made).
//!
//! Three answers, never more:
//! - [`Found::Key`]: some node gave a transaction whose recovered key has this address. That proves itself (the
//!   key is checked against the address here once more), so one node anywhere is enough; no agreement is asked.
//! - [`Found::NoAnchor`]: every node of every network read every block from the registry's start to its head
//!   and holds no anchor this address signed. Only all of them saying so is this answer.
//! - [`Found::Unread`]: no key was found, and some network was not read through (a node did not answer, broke off,
//!   served another chain, gave an answer that does not hold together, or a network had no node). Each node's
//!   words are the evidence.

use crate::node::{self, NodeAddr};
use zikaron::cryptox;

/// One network as it is read for a key: its chain, its registry, the registry's first block, its nodes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Net {
    pub chain: u64,
    pub registry: [u8; 20],
    pub from_block: u64,
    pub nodes: Vec<NodeAddr>,
}

/// What the chain says of an address's key. Closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
    /// The key (uncompressed, 65 bytes), whose address is the one asked about.
    Key([u8; 65]),
    /// Every network read through by every node: no anchor this address signed.
    NoAnchor,
    /// Not found, and not everything was read: what each node that did not read said.
    Unread(Vec<String>),
}

/// How one node's reading went.
enum Read {
    Key([u8; 65]),
    Nothing,
    Failed(String),
}

/// Find `author`'s public key on `nets`.
pub fn key_of(nets: &[Net], author: &[u8; 20]) -> Found {
    let mut asks: Vec<(u64, [u8; 20], u64, NodeAddr)> = Vec::new();
    let mut unread: Vec<String> = Vec::new();
    for n in nets {
        if n.nodes.is_empty() {
            unread.push(format!("{} NO_ENDPOINT", n.chain));
            continue;
        }
        let eps: Vec<node::Endpoint> = n.nodes.iter().map(|u| node::Endpoint::at(n.chain, u.clone())).collect();
        match node::serving(&eps, n.chain) {
            Ok((kept, left)) => {
                // A node serving another chain is a node of this network that did not read it.
                unread.extend(left.iter().map(|w| format!("{} {w:?}", n.chain)));
                asks.extend(kept.into_iter().map(|e| (n.chain, n.registry, n.from_block, e.url)));
            }
            Err(t) => unread.push(format!("{} {:?}", n.chain, t.code)),
        }
    }
    let read = zikaron_anchor::endpoints::each(
        asks,
        |(chain, registry, from, url)| read_one(chain, &registry, from, &url, author),
        || Read::Failed("SHORT".into()),
    );
    let mut found = None;
    for r in read {
        match r {
            Read::Key(k) if cryptox::address_of_pubkey(&k) == *author => {
                found.get_or_insert(k);
            }
            Read::Key(_) | Read::Nothing => {}
            Read::Failed(w) => unread.push(w),
        }
    }
    match found {
        Some(k) => Found::Key(k),
        None if unread.is_empty() => Found::NoAnchor,
        None => Found::Unread(unread),
    }
}

/// What a package's sealing takes from the chain for an address, as both ends answer it: the key (compressed), or a
/// refusal in the addressed package's one table (`zikaron_seal::addressed::Why`: `NotAnchored`, `ChainUnread` with
/// the evidence, `KeyShape` for a key found that is no point, which a recovered key never is).
pub type KeyFor = Result<[u8; 33], (zikaron_seal::addressed::Why, String)>;

/// The key behind `address` for sealing a package to it, both ends' one judgement: looked for on `nets` (the main
/// network, or the networks given) and on every read-only network of `readonly` that passes its fingerprint gate
/// against `pinned` (`fingerprint::gate_counted`), on the nodes that counted there; one that does not pass is a
/// network not read: no anchor is taken from it, and it never makes "not anchored". A read-only network that is one of
/// `nets` itself (same chain and registry; the read-only table is the machine's, the main network the home's, so a
/// copy can remain) is that network already: past its gate its nodes join it, and turned away it names nothing (as
/// the read side's windows take it, `app::widex`). `unread` names networks known not read before anything is asked.
/// A network that is not there at all (the main network with no chain set) is no network to read and counts for
/// nothing; with no network at all there is nothing read, and that is never "not anchored". Found, it proves itself;
/// every node of every network read through with no anchor the address signed is `NotAnchored`; otherwise
/// `ChainUnread` with each network's and node's words.
pub fn key_for(address: &[u8; 20], nets: Vec<Net>, readonly: &[Net], mut unread: Vec<String>, pinned: &str) -> KeyFor {
    use zikaron_seal::addressed::Why;
    let mut nets = nets;
    for n in readonly {
        let nodes: Vec<String> = n.nodes.iter().map(|u| u.for_transport().to_string()).collect();
        let (g, counted) = crate::fingerprint::gate_counted(n.chain, &n.registry, &nodes, pinned);
        let counted: Vec<NodeAddr> = n.nodes.iter().filter(|u| counted.iter().any(|c| c == u.for_transport())).cloned().collect();
        match nets.iter_mut().find(|m| m.chain == n.chain && m.registry == n.registry) {
            Some(m) if g.admits() => {
                m.from_block = m.from_block.min(n.from_block);
                for u in counted {
                    if !m.nodes.contains(&u) {
                        m.nodes.push(u);
                    }
                }
            }
            Some(_) => {}
            None if g.admits() => nets.push(Net { nodes: counted, ..n.clone() }),
            None => unread.push(format!("{} {}", n.chain, g.code())),
        }
    }
    if nets.is_empty() && unread.is_empty() {
        unread.push("NO_NETWORK".to_string());
    }
    match key_of(&nets, address) {
        Found::Key(k) => zikaron_seal::crypto::point(&k).ok_or((Why::KeyShape, String::new())),
        Found::NoAnchor if unread.is_empty() => Err((Why::NotAnchored, String::new())),
        Found::NoAnchor => Err((Why::ChainUnread, unread.join(" \u{b7} "))),
        Found::Unread(w) => {
            unread.extend(w);
            Err((Why::ChainUnread, unread.join(" \u{b7} ")))
        }
    }
}

/// One node: its head, then the anchor scan's reader from the registry's first block to that head.
fn read_one(chain: u64, registry: &[u8; 20], from: u64, url: &NodeAddr, author: &[u8; 20]) -> Read {
    let Some(mut ep) = node::endpoint_at(url.for_transport()) else {
        return Read::Failed(format!("{chain} {} NOT_ADDRESS", url.said()));
    };
    let head = match zikaron_anchor::send::head_of(ep.as_mut()) {
        Ok(h) => h,
        Err(t) => return Read::Failed(format!("{chain} {} eth_blockNumber {t:?}", url.said())),
    };
    match zikaron_anchor::scan::author_key(ep.as_mut(), chain, registry, author, from, head) {
        Ok(Some(k)) => Read::Key(k),
        Ok(None) => Read::Nothing,
        Err(r) => Read::Failed(format!("{chain} {} {} {}", url.said(), r.code(), r.detail())),
    }
}
