use crate::fault::{Fault, Known};
use crate::home::{Home, Slot};
use zikaron::json::Value;

pub use zikaron_seal::unfetched::{member, word, State, FILE};

pub fn fault(s: &State) -> Fault {
    match s {
        State::Unfetched => Fault::known(Known::LedgerNotFetched, String::new()),
        State::NewerElsewhere { missing, .. } => Fault::known(Known::NewerElsewhere, missing.to_string()),
    }
}

pub fn mark(home: &Home) -> Result<(), Fault> {
    write(home, &State::Unfetched)
}

pub fn write(home: &Home, s: &State) -> Result<(), Fault> {
    crate::local::put(&home.dir(Slot::Settings), FILE, crate::local::Doc::Unfetched, &zikaron_seal::unfetched::render(s))
}

pub fn read(home: &Home) -> Result<Option<State>, Fault> {
    let p = home.dir(Slot::Settings).join(FILE);
    let Some(bytes) = crate::local::read(&p, crate::local::Doc::Unfetched)? else { return Ok(None) };
    Ok(Some(zikaron_seal::unfetched::of_plain(&bytes, &p.display().to_string())?))
}

pub fn standing(home: &Home) -> Result<Option<State>, Fault> {
    match read(home)? {
        Some(State::NewerElsewhere { missing, lacks }) if !lacks.is_empty() => Ok(crate::sendx::mark_standing(Some(State::NewerElsewhere { missing, lacks }), &home.ledger()?.pile()?.items)),
        m => Ok(m),
    }
}

pub fn clear(home: &Home) -> Result<(), Fault> {
    let p = home.dir(Slot::Settings).join(FILE);
    match std::fs::remove_file(&p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(crate::fault::classify(&e, &p.display().to_string())),
    }
}

pub const READ_ON: &str = "unfetched-read-on.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct On {
    pub chain: u64,
    pub registry: [u8; 20],
}

impl On {
    pub fn of(s: &crate::settings::Settings) -> Option<On> {
        Some(On { chain: s.chain_id?, registry: s.registry?.0 })
    }

    pub fn asked(ask: &crate::exitgate::Ask) -> On {
        let lane = ask.anchoring();
        On { chain: lane.chain, registry: lane.registry }
    }
}

mod on_member {
    pub const CHAIN: &str = "chainId";
    pub const REGISTRY: &str = "registry";
}

pub fn passed_on(home: &Home, on: On) -> Result<(), Fault> {
    let v = Value::Obj(vec![(on_member::CHAIN.to_string(), Value::Str(on.chain.to_string())), (on_member::REGISTRY.to_string(), Value::Str(zikaron::hexfmt::encode(&on.registry)))]);
    crate::local::put(&home.dir(Slot::Settings), READ_ON, crate::local::Doc::Unfetched, &zikaron::json::canon_bytes(&v))
}

pub fn read_on(home: &Home) -> Result<Option<On>, Fault> {
    let p = home.dir(Slot::Settings).join(READ_ON);
    let Some(bytes) = crate::local::read(&p, crate::local::Doc::Unfetched)? else { return Ok(None) };
    let bad = || Fault::known(Known::SettingsShape, p.display().to_string());
    let v = zikaron::json::parse(&bytes).map_err(|_| bad())?;
    let chain = v.member(on_member::CHAIN).and_then(Value::as_str).and_then(|c| c.parse::<u64>().ok()).ok_or_else(bad)?;
    let registry = v.member(on_member::REGISTRY).and_then(Value::as_str).and_then(crate::key::Address::parse).ok_or_else(bad)?;
    Ok(Some(On { chain, registry: registry.0 }))
}

pub fn forget_on(home: &Home) -> Result<(), Fault> {
    let p = home.dir(Slot::Settings).join(READ_ON);
    match std::fs::remove_file(&p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(crate::fault::classify(&e, &p.display().to_string())),
    }
}

pub fn moved(home: &Home, now: Option<On>) -> bool {
    match read_on(home) {
        Ok(Some(on)) => now != Some(on),
        Ok(None) => false,
        Err(_) => true,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tail {
    Pass { anchors: usize },
    NewerElsewhere { missing: usize, anchors: usize, lacks: Vec<String> },
}

pub fn tail_check(pile: &[Vec<u8>], fragment: &Value) -> Tail {
    let ids: Vec<String> = pile.iter().filter_map(|b| zikaron_glue::book::check(b).ok()).map(|e| e.id_hex().trim_start_matches("0x").to_ascii_lowercase()).collect();
    let hashes: Vec<String> = match fragment {
        Value::Obj(m) => match m.iter().find(|(k, _)| k == "anchors").map(|(_, v)| v) {
            Some(Value::Arr(a)) => a
                .iter()
                .filter_map(|x| match x {
                    Value::Obj(r) => r.iter().find(|(k, _)| k == "hash").and_then(|(_, v)| if let Value::Str(s) = v { Some(s.trim_start_matches("0x").to_ascii_lowercase()) } else { None }),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    };
    let lacks: Vec<String> = hashes.iter().filter(|h| !ids.contains(h)).cloned().collect();
    if lacks.is_empty() {
        Tail::Pass { anchors: hashes.len() }
    } else {
        Tail::NewerElsewhere { missing: lacks.len(), anchors: hashes.len(), lacks }
    }
}

pub fn land(home: &Home, items: &[Vec<u8>]) -> Result<usize, Fault> {
    let ledger = home.ledger()?;
    let mut have: Vec<[u8; 32]> = ledger.pile()?.items.iter().filter_map(|b| zikaron_glue::book::check(b).ok()).map(|e| e.id).collect();
    let nk = crate::names::key()?;
    let mut sealed: Vec<(zikaron_store::EntryName, Vec<u8>)> = Vec::new();
    for b in items {
        let e = zikaron_glue::book::check(b).map_err(|t| Fault::known(Known::EntryRefused, format!("{t:?}")))?;
        if have.contains(&e.id) {
            continue;
        }
        let name = crate::local::entry_name_under(&nk, &e.id_hex())?;
        have.push(e.id);
        sealed.push((name, ledger.seal_entry(b)?));
    }
    let mut staged = Vec::new();
    for (name, bytes) in &sealed {
        match ledger.store().stage(name, bytes) {
            Ok(s) => staged.push(s),
            Err(t) => {
                for s in staged {
                    s.abandon();
                }
                return Err(Fault::known(Known::Ledger, format!("{t:?}")));
            }
        }
    }
    let mut n = 0;
    let mut rest = staged.into_iter();
    while let Some(s) = rest.next() {
        if let Err(t) = s.link().and_then(|l| l.seal()) {
            for left in rest {
                left.abandon();
            }
            return Err(Fault::known(Known::RestorePartial, format!("{t:?}")));
        }
        n += 1;
    }
    Ok(n)
}
