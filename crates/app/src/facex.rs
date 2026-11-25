pub use crate::chainx::{FaceBuilt as Built, FaceLane as Lane, FaceLeft as Left, FaceNotRead as NotRead};

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Unread,
    Read(Box<Built>),
    NotRead(Missing),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Missing {
    NoRoot,
    NoChain(Vec<(u64, Left)>),
    NoTable(Vec<(u64, Left)>),
}

impl From<NotRead> for Missing {
    fn from(n: NotRead) -> Missing {
        match n {
            NotRead::NoChain(l) => Missing::NoChain(l),
            NotRead::NoTable(l) => Missing::NoTable(l),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Book {
    Off,
    Wrong,
    Unread(crate::fault::Fault),
    Read { under: Option<[u8; 20]>, plan: Result<crate::rotatex::Enrol, crate::rotatex::Closed> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Books {
    pub root: crate::key::Address,
    pub office: crate::key::Address,
    pub root_key_here: bool,
    pub chains: Vec<(u64, Book)>,
}

fn state_of(r: Result<Built, NotRead>) -> State {
    match r {
        Ok(b) => State::Read(Box::new(b)),
        Err(n) => State::NotRead(n.into()),
    }
}

pub fn own(eps: &[crate::chainx::Endpoint], chains: &[(u64, crate::key::Address, u64)], items: &[Vec<u8>]) -> State {
    crate::trace::mark(crate::feature::Feature::H2);
    let Some(root) = zikaron_glue::book::root_of(items).ok().and_then(|r| crate::key::Address::parse(&r)) else { return State::NotRead(Missing::NoRoot) };
    let lanes: Vec<Lane> = chains.iter().map(|(chain, registry, from_block)| Lane { chain: *chain, registry: registry.0, from_block: *from_block }).collect();
    state_of(crate::chainx::face_input(eps, &root.0, &lanes, items, &crate::sendx::Local))
}

pub fn other(
    eps: &[crate::chainx::Endpoint],
    ground: &crate::auditx::Ground,
    nets: &[crate::readnets::Net],
    missed: &[crate::widex::Missed],
    who: &crate::key::Address,
    bytes: &[Vec<u8>],
) -> State {
    let (all, lanes) = other_lanes(eps, ground, nets, missed);
    state_of(crate::chainx::face_input(&all, &who.0, &lanes, bytes, &crate::sendx::Local))
}

fn other_lanes(eps: &[crate::chainx::Endpoint], ground: &crate::auditx::Ground, nets: &[crate::readnets::Net], missed: &[crate::widex::Missed]) -> (Vec<crate::chainx::Endpoint>, Vec<Lane>) {
    let mut lanes = vec![Lane { chain: ground.chain, registry: ground.registry.0, from_block: ground.from_block }];
    lanes.extend(nets.iter().map(|n| Lane { chain: n.chain_id, registry: n.registry.0, from_block: n.from_block }));
    lanes.retain(|l| !missed.iter().any(|m| m.chain_id == l.chain && m.registry.0 == l.registry));
    (crate::readnets::all_endpoints(eps, nets), lanes)
}

pub const MOVES_MAX: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub struct Hop {
    pub root: crate::key::Address,
    pub state: State,
    pub joined: Option<bool>,
    pub again: bool,
}

pub fn other_moves(eps: &[crate::chainx::Endpoint], ground: &crate::auditx::Ground, nets: &[crate::readnets::Net], missed: &[crate::widex::Missed], who: &crate::key::Address, first: &State) -> Vec<Hop> {
    let (all, lanes) = other_lanes(eps, ground, nets, missed);
    let mut seen = vec![*who];
    let mut out: Vec<Hop> = Vec::new();
    let mut at = first.clone();
    while out.len() < MOVES_MAX {
        let State::Read(b) = &at else { break };
        let Some(to) = b.table.reroot.as_ref().and_then(|l| crate::key::Address::parse(&l.other)) else { break };
        if seen.contains(&to) {
            out.push(Hop { root: to, state: State::Unread, joined: None, again: true });
            break;
        }
        seen.push(to);
        let next = state_of(crate::chainx::face_input(&all, &to.0, &lanes, &[], &crate::sendx::Local));
        let joined = match &next {
            State::Read(n) => Some(zikaron_glue::face::joined(&b.table, &n.table)),
            _ => None,
        };
        out.push(Hop { root: to, state: next.clone(), joined, again: false });
        if joined != Some(true) {
            break;
        }
        at = next;
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Old,
    New,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pair {
    pub side: Side,
    pub other: crate::key::Address,
    pub other_id: Option<String>,
    pub other_chain: Option<u64>,
    pub state: State,
}

impl Pair {
    pub fn joined(&self, own: &State) -> Option<bool> {
        let (State::Read(mine), State::Read(theirs)) = (own, &self.state) else { return None };
        Some(match self.side {
            Side::Old => zikaron_glue::face::joined(&mine.table, &theirs.table),
            Side::New => zikaron_glue::face::joined(&theirs.table, &mine.table),
        })
    }

    pub fn table(&self) -> Option<&zikaron_glue::face::Table> {
        match &self.state {
            State::Read(b) => Some(&b.table),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Joined {
    pub first: Pair,
    pub second: Pair,
}

impl Joined {
    pub fn joined(&self) -> Option<bool> {
        match (self.first.table(), self.second.table()) {
            (Some(a), Some(b)) => Some(zikaron_glue::face::joined(a, b)),
            _ => None,
        }
    }
}

pub fn other_of(table: Option<&zikaron_glue::face::Table>, items: &[Vec<u8>], root: &crate::key::Address, reg: Option<&crate::identity::Registry>) -> Option<(Side, crate::key::Address, Option<String>)> {
    let local = |a: &crate::key::Address| reg.and_then(|r| r.rows.iter().find(|row| row.address(crate::roles::Role::Author) == Some(*a)).map(|row| row.id.clone()));
    let with = |side: Side, a: crate::key::Address| Some((side, a, local(&a)));
    if let Some(a) = table.and_then(|t| t.reroot.as_ref()).and_then(|l| crate::key::Address::parse(&l.other)) {
        return with(Side::Old, a);
    }
    if let Some(a) = zikaron_glue::facegate::written_reroot(items).and_then(|(_, to)| crate::key::Address::parse(&to)) {
        return with(Side::Old, a);
    }
    if let Some(row) = reg.and_then(|r| r.rows.iter().find(|r| r.reroot_of == Some(*root))) {
        if let Some(a) = row.address(crate::roles::Role::Author) {
            return Some((Side::Old, a, Some(row.id.clone())));
        }
    }
    if let Some(a) = table.and_then(|t| t.origin.as_ref()).and_then(|l| crate::key::Address::parse(&l.other)) {
        return with(Side::New, a);
    }
    let reg = reg?;
    let mine = reg.rows.iter().find(|r| r.address(crate::roles::Role::Author) == Some(*root))?;
    mine.reroot_of.and_then(|o| with(Side::New, o))
}

pub fn pair_read(eps: &[crate::chainx::Endpoint], chains: &[(u64, crate::key::Address, u64)], side: Side, other: crate::key::Address, other_id: Option<String>) -> Pair {
    let home = other_id.as_deref().and_then(|_| {
        let reg = crate::register::view().ok()?;
        let row = reg.rows.iter().find(|r| r.address(crate::roles::Role::Author) == Some(other))?;
        let dir = row.home(crate::roles::Role::Author)?;
        crate::home::Home::open(dir).ok().and_then(|h| crate::settings::Settings::read(&h).ok())
    });
    let (lanes, nodes, other_chain): (Vec<Lane>, Vec<crate::chainx::Endpoint>, Option<u64>) = match &home {
        Some(s) => {
            let mut lanes: Vec<Lane> = Vec::new();
            if let (Some(c), Some(r)) = (s.chain_id, s.registry) {
                lanes.push(Lane { chain: c, registry: r.0, from_block: s.from_block });
            }
            for e in &s.earlier {
                if !lanes.iter().any(|l| l.chain == e.chain_id) {
                    lanes.push(Lane { chain: e.chain_id, registry: e.registry, from_block: e.from_block });
                }
            }
            (lanes, crate::chainx::nodes_of(s), s.chain_id)
        }
        None => (chains.iter().map(|(chain, registry, from_block)| Lane { chain: *chain, registry: registry.0, from_block: *from_block }).collect(), eps.to_vec(), None),
    };
    let state = if lanes.is_empty() { State::NotRead(Missing::NoChain(Vec::new())) } else { state_of(crate::chainx::face_input(&nodes, &other.0, &lanes, &[], &crate::sendx::Local)) };
    Pair { side, other, other_id, other_chain, state }
}

pub fn hint(t: &zikaron_glue::face::Table) -> Option<String> {
    (t.unproven > 0).then(|| crate::lang::fill1(crate::lang::Key::FaceUnprovenHint, &t.unproven.to_string()))
}

pub fn left_said(l: &Left) -> String {
    use crate::lang::{t, Key};
    let with = |k: Key, tr: &crate::chainx::SendTrouble| format!("{}: {}", t(k), crate::fault::Fault::from(tr.clone()).human());
    match l {
        Left::NoBook => t(Key::FaceLeftNoBook).to_string(),
        Left::BookWrong => t(Key::FaceLeftBookWrong).to_string(),
        Left::BookUnread(tr) => with(Key::FaceLeftBookUnread, tr),
        Left::HeadUnread(tr) => with(Key::FaceLeftHeadUnread, tr),
        Left::NoBlockBy => t(Key::FaceLeftNoBlock).to_string(),
        Left::Unread(tr) => with(Key::FaceLeftUnread, tr),
    }
}

pub fn missing_machine(m: &Missing) -> String {
    match m {
        Missing::NoRoot => crate::chainx::FACE_NO_ROOT.to_string(),
        Missing::NoChain(l) => NotRead::NoChain(l.clone()).said(),
        Missing::NoTable(l) => NotRead::NoTable(l.clone()).said(),
    }
}

pub fn missing_said(m: &Missing) -> String {
    use crate::lang::{t, Key};
    match m {
        Missing::NoRoot => t(Key::FaceNoRoot).to_string(),
        Missing::NoChain(_) => t(Key::FaceNoChain).to_string(),
        Missing::NoTable(_) => t(Key::FaceNoTable).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_root_and_no_chain_read_are_no_table_said_by_name() {
        assert_eq!(own(&[], &[], &[]), State::NotRead(Missing::NoRoot));
        let st = state_of(crate::chainx::face_input(&[], &[0x11; 20], &[Lane { chain: 31_337, registry: [0x22; 20], from_block: 0 }], &[], &crate::sendx::Local));
        assert_eq!(st, State::NotRead(Missing::NoChain(vec![(31_337, Left::NoBook)])));
        assert_eq!(missing_said(&Missing::NoRoot), crate::lang::t(crate::lang::Key::FaceNoRoot));
        assert_eq!(left_said(&Left::NoBook), crate::lang::t(crate::lang::Key::FaceLeftNoBook));
        assert_eq!(State::NotRead(NotRead::NoTable(Vec::new()).into()), State::NotRead(Missing::NoTable(Vec::new())));
    }

    #[test]
    fn the_line_to_read_again_comes_with_unproven_records_alone() {
        let input = zikaron_glue::face::input(&[0x11; 20], &[], &[], &[], &[]);
        let mut t = zikaron_glue::face::read(&input).expect("a table");
        t.unread = 7;
        assert_eq!(hint(&t), None);
        t.unproven = 2;
        assert_eq!(hint(&t), Some(crate::lang::fill1(crate::lang::Key::FaceUnprovenHint, "2")));
    }

    #[test]
    fn a_display_name_is_laid_only_where_the_table_says() {
        const ALLOWED: &[&str] = &["window/persona.rs", "window/others.rs"];
        let roots = [concat!(env!("CARGO_MANIFEST_DIR"), "/src"), concat!(env!("CARGO_MANIFEST_DIR"), "/../zikaron-ui/src")];
        let mut found: Vec<std::path::PathBuf> = Vec::new();
        let mut stack: Vec<std::path::PathBuf> = roots.iter().map(std::path::PathBuf::from).collect();
        while let Some(p) = stack.pop() {
            let Ok(md) = std::fs::metadata(&p) else { continue };
            if md.is_dir() {
                stack.extend(std::fs::read_dir(&p).into_iter().flatten().flatten().map(|e| e.path()));
            } else if p.extension().is_some_and(|x| x == "rs") && !p.ends_with("facex.rs") {
                let text = std::fs::read_to_string(&p).unwrap_or_default();
                if text.contains(".lay_alone()") || text.contains(".copy_name()") {
                    found.push(p);
                }
            }
        }
        found.retain(|f| !ALLOWED.iter().any(|a| f.ends_with(a)));
        assert!(found.is_empty(), "a display name laid or copied outside the named places: {found:?}");
    }
}
