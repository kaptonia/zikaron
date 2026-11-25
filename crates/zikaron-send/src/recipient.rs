use crate::node::{self, NodeAddr};
use zikaron::cryptox;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Net {
    pub chain: u64,
    pub registry: [u8; 20],
    pub from_block: u64,
    pub nodes: Vec<NodeAddr>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
    Key([u8; 65]),
    NoAnchor,
    Unread(Vec<String>),
    Unproven(Vec<String>),
}

enum Read {
    Key([u8; 65]),
    Nothing,
    Failed(String),
    Aside(String),
    Wide(String, bool),
    Unproven(String),
}

pub trait Keys {
    fn kept(&self, address: &[u8; 20]) -> Option<[u8; 65]>;
    fn keep(&self, address: &[u8; 20], key: &[u8; 65]);
}

impl<R: crate::checked::Room> Keys for crate::checked::Record<R> {
    fn kept(&self, address: &[u8; 20]) -> Option<[u8; 65]> {
        crate::checked::key_of(&self.0, address)
    }

    fn keep(&self, address: &[u8; 20], key: &[u8; 65]) {
        let _ = crate::checked::add_key(&self.0, address, key);
    }
}

pub fn key_of(nets: &[Net], author: &[u8; 20]) -> Found {
    let mut asks: Vec<(u64, [u8; 20], u64, NodeAddr)> = Vec::new();
    let mut net_of: Vec<usize> = Vec::new();
    let mut unread: Vec<String> = Vec::new();
    for (i, n) in nets.iter().enumerate() {
        if n.nodes.is_empty() {
            unread.push(format!("{} NO_ENDPOINT", n.chain));
            continue;
        }
        let eps: Vec<node::Endpoint> = n.nodes.iter().map(|u| node::Endpoint::at(n.chain, u.clone())).collect();
        match node::serving(&eps, n.chain) {
            Ok((kept, left)) => {
                unread.extend(left.iter().map(|w| format!("{} {w:?}", n.chain)));
                net_of.extend(kept.iter().map(|_| i));
                asks.extend(kept.into_iter().map(|e| (n.chain, n.registry, n.from_block, e.url)));
            }
            Err(t) => unread.push(format!("{} {:?}", n.chain, t.code)),
        }
    }
    let who = *author;
    let (places, nets_of): (Vec<String>, Vec<usize>) = (asks.iter().map(|(_, _, _, u)| u.for_transport().to_string()).collect(), net_of.clone());
    let kept_asks = asks.clone();
    let read: Vec<Option<Read>> = zikaron_anchor::endpoints::until_enough(
        asks,
        move |(chain, registry, from, url)| read_one(chain, &registry, from, &url, &who),
        || Read::Failed("SHORT".into()),
        |slots: &[Option<Read>]| {
            if slots.iter().any(|r| matches!(r, Some(Read::Key(k)) if cryptox::address_of_pubkey(k) == who)) {
                return true;
            }
            let mut nets: Vec<usize> = nets_of.clone();
            nets.sort_unstable();
            nets.dedup();
            nets.iter().all(|n| {
                let nothing = places.iter().zip(&nets_of).zip(slots).filter(|((_, m), r)| *m == n && matches!(r, Some(Read::Nothing) | Some(Read::Key(_)))).map(|((p, _), _)| p.as_str());
                zikaron_anchor::endpoints::distinct_places(nothing) >= 2
            })
        },
    );
    let mut read: Vec<Read> = read.into_iter().map(|r| r.unwrap_or(Read::Nothing)).collect();
    let mut nets_read: Vec<usize> = net_of.clone();
    nets_read.sort_unstable();
    nets_read.dedup();
    for n in &nets_read {
        let at: Vec<usize> = (0..read.len()).filter(|i| net_of[*i] == *n).collect();
        let mut sub: Vec<Read> = at.iter().map(|i| std::mem::replace(&mut read[*i], Read::Nothing)).collect();
        zikaron_anchor::action::cut_and_ask(
            &mut sub,
            |j, _| vec![zikaron_anchor::endpoints::place_of(kept_asks[at[j]].3.for_transport())],
            |r| matches!(r, Read::Key(_) | Read::Nothing | Read::Unproven(_)),
            |r| matches!(r, Read::Wide(..)),
            |idx, _| {
                let asks: Vec<_> = idx.iter().map(|j| kept_asks[at[*j]].clone()).collect();
                zikaron_anchor::endpoints::until_enough(asks, move |(chain, registry, from, url)| read_one(chain, &registry, from, &url, &who), || Read::Failed("SHORT".into()), |slots: &[Option<Read>]| slots.iter().flatten().any(|r| matches!(r, Read::Key(_) | Read::Nothing | Read::Unproven(_))))
                    .into_iter()
                    .map(|r| r.unwrap_or(Read::Aside("SHORT".into())))
                    .collect()
            },
        );
        for (j, i) in at.iter().enumerate() {
            read[*i] = std::mem::replace(&mut sub[j], Read::Nothing);
        }
    }
    let mut found = None;
    let mut unproven = false;
    let mut answered: Vec<usize> = Vec::new();
    let mut aside: Vec<(usize, String)> = Vec::new();
    for (net, r) in net_of.into_iter().zip(read) {
        match r {
            Read::Key(k) if cryptox::address_of_pubkey(&k) == *author => {
                answered.push(net);
                found.get_or_insert(k);
            }
            Read::Key(_) | Read::Nothing => answered.push(net),
            Read::Aside(w) | Read::Wide(w, true) => aside.push((net, w)),
            Read::Failed(w) | Read::Wide(w, false) => unread.push(w),
            Read::Unproven(w) => {
                unproven = true;
                unread.insert(0, w);
            }
        }
    }
    unread.extend(aside.into_iter().filter(|(c, _)| !answered.contains(c)).map(|(_, w)| w));
    match found {
        Some(k) => Found::Key(k),
        None if unproven => Found::Unproven(unread),
        None if unread.is_empty() => Found::NoAnchor,
        None => Found::Unread(unread),
    }
}

pub type KeyFor = Result<[u8; 33], (zikaron_seal::addressed::Why, String)>;

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
        Found::Unproven(w) => {
            unread.extend(w);
            Err((Why::Unproven, unread.join(" \u{b7} ")))
        }
    }
}

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
        Err(r @ (zikaron_anchor::scan::Refusal::TxNotItself(_) | zikaron_anchor::scan::Refusal::Malformed(zikaron_anchor::scan::Shape::TxLacks { .. })))
            if !matches!(&r, zikaron_anchor::scan::Refusal::Malformed(zikaron_anchor::scan::Shape::TxLacks { part, .. }) if part == "transaction") =>
        {
            Read::Unproven(format!("{chain} {} {} {}", url.said(), r.code(), r.detail()))
        }
        Err(r) => {
            let words = format!("{chain} {} {} {}", url.said(), r.code(), r.detail());
            if matches!(r, zikaron_anchor::scan::Refusal::Unanswered { asked: zikaron_anchor::scan::Asked::Short, .. }) {
                Read::Aside(words)
            } else if r.cut() {
                Read::Wide(words, r.stands_aside())
            } else if r.stands_aside() {
                Read::Aside(words)
            } else {
                Read::Failed(words)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum From {
    Ledger,
    Book,
    Root,
}

pub fn book_current(root: &[u8; 20], nets: &[Net]) -> (Option<[u8; 20]>, Vec<String>) {
    let mut unread = Vec::new();
    for n in nets {
        if zikaron_anchor::book::at(n.chain).is_none() {
            continue;
        }
        let eps: Vec<node::Endpoint> = n.nodes.iter().map(|u| node::Endpoint::at(n.chain, u.clone())).collect();
        crate::book::ahead(&eps, n.chain, &[(zikaron_anchor::book::Call::Current, *root)], &|_| Vec::new());
        match crate::book::state(&eps, n.chain) {
            Ok(crate::book::State::At(at)) => match crate::book::address(&eps, n.chain, &at, zikaron_anchor::book::Call::Current, root) {
                Ok(k) if k != [0u8; 20] && k != *root => return (Some(k), unread),
                Ok(_) => {}
                Err(t) => unread.push(format!("{} BOOK {:?}", n.chain, t.code)),
            },
            Ok(crate::book::State::Off) => {}
            Ok(crate::book::State::Wrong) => unread.push(format!("{} BOOK_CODE", n.chain)),
            Err(t) => unread.push(format!("{} BOOK {:?}", n.chain, t.code)),
        }
    }
    (None, unread)
}

pub fn held(root: &[u8; 20], seats: &[([u8; 20], [u8; 20])], kept: &[Vec<Vec<u8>>]) -> Option<[u8; 20]> {
    if let Some((_, current)) = seats.iter().find(|(r, _)| r == root) {
        return Some(*current);
    }
    let hex = zikaron::hexfmt::encode(root);
    kept.iter().filter(|ledger| zikaron_glue::book::root_of(ledger).is_ok_and(|r| r.eq_ignore_ascii_case(&hex))).find_map(|ledger| {
        let read = zikaron_glue::book::read_under(&hex, ledger, &zikaron_glue::book::empty_fragment()).ok()?;
        read.office.as_deref().and_then(zikaron::hexfmt::decode).and_then(|b| <[u8; 20]>::try_from(b).ok())
    })
}

pub fn key_for_root(root: &[u8; 20], held: Option<[u8; 20]>, nets: Vec<Net>, readonly: &[Net], unread: Vec<String>, pinned: &str, keys: Option<&dyn Keys>) -> Result<([u8; 33], From), (zikaron_seal::addressed::Why, String)> {
    let mut offer = nets.clone();
    offer.extend(readonly.iter().cloned());
    let (current, from) = match book_current(root, &offer) {
        (Some(k), _) => (k, From::Book),
        (None, missed) if !missed.is_empty() => return Err((zikaron_seal::addressed::Why::ChainUnread, missed.join(" \u{b7} "))),
        (None, _) => match held {
            Some(k) => (k, From::Ledger),
            None => (*root, From::Root),
        },
    };
    if let Some(k) = keys.and_then(|s| s.kept(&current)) {
        return zikaron_seal::crypto::point(&k).map(|p| (p, from)).ok_or((zikaron_seal::addressed::Why::KeyShape, String::new()));
    }
    let found = key_for(&current, nets, readonly, unread, pinned)?;
    if let (Some(s), Some(full)) = (keys, zikaron_seal::crypto::uncompressed(&found)) {
        s.keep(&current, &full);
    }
    Ok((found, from))
}
