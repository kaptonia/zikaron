use crate::node::Endpoint;
use crate::{Code, End, Facts, Trouble, Why};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Exit {
    Send,
    Kit,
    GrantFile,
    Mirror,
    Badge,
}

impl Exit {
    pub const ALL: [Exit; 5] = [Exit::Send, Exit::Kit, Exit::GrantFile, Exit::Mirror, Exit::Badge];
}

pub trait Held: Facts {
    type Fail: From<Trouble>;
    fn pile(&self) -> Result<Vec<Vec<u8>>, Self::Fail>;
    fn behind(&self, lacks: &[String]) -> Self::Fail;
    fn unread(&self, f: Self::Fail) -> Self::Fail;
}

impl<E: End + Facts> Held for E {
    type Fail = E::Fail;
    fn pile(&self) -> Result<Vec<Vec<u8>>, E::Fail> {
        End::pile(self)
    }
    fn behind(&self, lacks: &[String]) -> E::Fail {
        End::behind(self, lacks)
    }
    fn unread(&self, f: E::Fail) -> E::Fail {
        End::unread(self, f)
    }
}

#[derive(Clone, Debug)]
pub struct Ask {
    pub root: PathBuf,
    pub lanes: Vec<Lane>,
    pub own: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Lane {
    pub chain: u64,
    pub registry: [u8; 20],
    pub from_block: u64,
    pub eps: Vec<Endpoint>,
}

impl Ask {
    pub fn anchoring(&self) -> &Lane {
        &self.lanes[0]
    }

    pub fn from_block_of(&self, chain: u64) -> Option<u64> {
        self.lanes.iter().find(|l| l.chain == chain).map(|l| l.from_block)
    }
}

pub fn ask(root: PathBuf, ground: &zikaron_seal::ground::Ground, eps: &[Endpoint], own: Option<String>) -> Result<Ask, Trouble> {
    let chain = ground.chain_id.ok_or_else(|| Trouble::text(Code::NoChainId, ""))?;
    let registry = ground.registry.ok_or_else(|| Trouble::text(Code::NoRegistry, ""))?;
    let of = |c: u64| -> Vec<Endpoint> { eps.iter().filter(|e| e.chain == c).cloned().collect() };
    let first = Lane { chain, registry, from_block: ground.from_block, eps: of(chain) };
    if first.eps.is_empty() {
        return Err(Trouble::text(Code::NoEndpoint, chain.to_string()));
    }
    let mut lanes = vec![first];
    for c in ground.other_chains() {
        if let Some((registry, from_block)) = ground.window_of(c) {
            lanes.push(Lane { chain: c, registry, from_block, eps: of(c) });
        }
    }
    Ok(Ask { root, lanes, own })
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reading {
    pub senders: Vec<String>,
    pub anchored: Vec<String>,
    pub on_chain: Vec<(u64, String)>,
    pub verified: Vec<String>,
    pub missing: Vec<String>,
    pub unread: Vec<Why>,
}

fn bare(s: &str) -> String {
    s.trim().trim_start_matches("0x").to_ascii_lowercase()
}

pub fn read<H: Held>(ask: &Ask, end: &H) -> Result<Reading, H::Fail> {
    let pile = end.pile()?;
    let mut verified: Vec<String> = pile.iter().filter_map(|b| zikaron_glue::book::check(b).ok()).map(|e| bare(&e.id_hex())).collect();
    verified.sort();
    verified.dedup();
    let moved_to = moved_to(ask, &pile);
    let senders = lineage(&pile, ask.own.as_deref(), moved_to.as_deref());
    let (mut anchors, mut chains, mut unread) = (Vec::new(), Vec::new(), Vec::new());
    for (i, lane) in ask.lanes.iter().enumerate() {
        match read_lane(lane, &senders, end) {
            Ok(fragment) => {
                for (into, member) in [(&mut anchors, fragment.member("anchors")), (&mut chains, fragment.member("basis").and_then(|b| b.member("chains")))] {
                    if let Some(zikaron::json::Value::Arr(xs)) = member {
                        into.extend(xs.iter().cloned());
                    }
                }
            }
            Err(t) if i == 0 || t.code == Code::AuditInput => return Err(t.into()),
            Err(t) => unread.push(Why::AtChain { chain: lane.chain, why: Box::new(Why::Each(vec![Why::Text(t.code.as_str().to_string()), t.why])) }),
        }
    }
    let fragment = zikaron::json::Value::Obj(vec![
        ("anchors".to_string(), zikaron::json::Value::Arr(anchors)),
        ("basis".to_string(), zikaron::json::Value::Obj(vec![("chains".to_string(), zikaron::json::Value::Arr(chains))])),
    ]);
    let mut on_chain: Vec<(u64, String)> = match fragment.member("anchors") {
        Some(zikaron::json::Value::Arr(a)) => a
            .iter()
            .filter_map(|x| match (x.member("chainId"), x.member("hash").and_then(zikaron::json::Value::as_str)) {
                (Some(zikaron::json::Value::Int(c)), Some(h)) => Some((*c, bare(h))),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    on_chain.sort();
    on_chain.dedup();
    let mut anchored: Vec<String> = on_chain.iter().map(|(_, h)| h.clone()).collect();
    anchored.sort();
    anchored.dedup();
    let mut owners: Vec<&str> = ask.own.as_deref().into_iter().collect();
    owners.extend(moved_to.as_deref());
    let missing = missing_of(&pile, &fragment, &owners, &anchored, &verified);
    Ok(Reading { senders, anchored, on_chain, verified, missing, unread })
}

pub fn lineage(pile: &[Vec<u8>], own: Option<&str>, moved_to: Option<&str>) -> Vec<String> {
    let mut senders: Vec<String> = pile.iter().filter_map(|b| zikaron_glue::book::check(b).ok()).map(|e| e.author).collect();
    senders.extend(own.map(str::to_string));
    senders.extend(moved_to.map(str::to_string));
    let mut senders: Vec<String> = senders.iter().map(|s| format!("0x{}", bare(s))).collect();
    senders.sort();
    senders.dedup();
    senders
}

pub fn lineage_bytes(lineage: &[String]) -> Vec<[u8; 20]> {
    lineage.iter().filter_map(|s| zikaron::hexfmt::decode(s).and_then(|b| b.try_into().ok())).collect()
}

fn book_office(own: Option<&str>, lanes: &[Lane]) -> Option<String> {
    let own: [u8; 20] = zikaron::hexfmt::decode(&format!("0x{}", bare(own?)))?.try_into().ok()?;
    lanes.iter().find_map(|l| {
        crate::book::ahead(&l.eps, l.chain, &[(zikaron_anchor::book::Call::Current, own)], &|_| Vec::new());
        let crate::book::State::At(at) = crate::book::state(&l.eps, l.chain).ok()? else { return None };
        let k = crate::book::address(&l.eps, l.chain, &at, zikaron_anchor::book::Call::Current, &own).ok()?;
        (k != own).then(|| zikaron::hexfmt::encode(&k))
    })
}

pub fn moved_to(ask: &Ask, pile: &[Vec<u8>]) -> Option<String> {
    book_office(ask.own.as_deref(), &ask.lanes).filter(|k| zikaron_glue::book::behind_book(pile, Some(k)))
}

fn read_lane(lane: &Lane, senders: &[String], facts: &dyn Facts) -> Result<zikaron::json::Value, Trouble> {
    if lane.eps.is_empty() {
        return Err(Trouble::text(Code::NoEndpoint, lane.chain.to_string()));
    }
    let (eps, left) = crate::batch::serving_eps(&lane.eps, lane.chain)?;
    let (head, _) = crate::node::head_block(&eps, lane.chain).map_err(|t| crate::batch::with_left(t, &left))?;
    let registries: Vec<String> = zikaron_anchor::revoke::registries(lane.chain, &[lane.registry]).iter().map(|r| zikaron::hexfmt::encode(r)).collect();
    let window = zikaron_glue::basis::Window { chain: lane.chain, registries: &registries, from_block: lane.from_block, to_block: head.max(lane.from_block), senders };
    let basis = zikaron::json::canon_bytes(&zikaron_glue::basis::basis_of(&window));
    match crate::agree::scan(&eps, &basis, facts).map_err(|t| crate::batch::with_left(t, &left))? {
        crate::agree::Scan::Basis(a) => Ok(a.fragment),
        crate::agree::Scan::NoLabel { .. } => Err(Trouble::text(Code::AuditInput, "")),
    }
}

pub fn missing_of(pile: &[Vec<u8>], fragment: &zikaron::json::Value, owners: &[&str], anchored: &[String], verified: &[String]) -> Vec<String> {
    let Ok(read) = zikaron_glue::book::read(pile, fragment) else {
        return anchored.iter().filter(|h| verified.binary_search(h).is_err()).cloned().collect();
    };
    let hash = |r: &zikaron::json::Value, k: &str| r.member(k).and_then(zikaron::json::Value::as_str).map(bare);
    let in_pile: std::collections::BTreeSet<String> = pile.iter().map(|b| zikaron::hexfmt::encode(&zikaron_glue::book::entry_id(b))).map(|h| bare(&h)).collect();
    let mut out: Vec<String> = read.rows("missing").iter().filter_map(|r| hash(r, "hash").or_else(|| r.as_str().map(bare))).chain(read.rows("unproven").iter().filter_map(|r| hash(r, "hash"))).filter(|h| !in_pile.contains(h)).collect();
    for own in owners.iter().map(|o| bare(o)).filter(|o| !read.lineage.iter().any(|k| bare(k) == *o)) {
        out.extend(read.rows("discarded").iter().filter(|r| hash(r, "sender").as_deref() == Some(own.as_str())).filter_map(|r| hash(r, "hash")));
    }
    out.sort();
    out.dedup();
    out
}

pub fn tail<H: Held>(ask: &Ask, end: &H) -> Result<(Vec<String>, usize), H::Fail> {
    let r = read(ask, end)?;
    Ok((r.missing, r.anchored.len()))
}

#[derive(Clone, Debug)]
pub struct Pass {
    reading: Reading,
    root: PathBuf,
}

impl Pass {
    pub fn reading(&self) -> &Reading {
        &self.reading
    }

    pub fn root(&self) -> &std::path::Path {
        &self.root
    }
}

pub fn pass<H: Held>(ask: &Ask, end: &H) -> Result<Pass, H::Fail> {
    let r = read(ask, end).map_err(|f| end.unread(f))?;
    if !r.missing.is_empty() {
        return Err(end.behind(&r.missing));
    }
    Ok(Pass { reading: r, root: ask.root.clone() })
}

