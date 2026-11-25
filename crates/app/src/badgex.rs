use crate::fault::{Fault, Known};
use crate::home::Home;
use std::path::{Path, PathBuf};
use zikaron_glue::book::Entry;

fn upstream_of(e: &Entry) -> Option<String> {
    match &e.body {
        zikaron::json::Value::Obj(m) => m.iter().find(|(k, _)| k == "upstream").and_then(|(_, v)| match v {
            zikaron::json::Value::Str(s) => Some(s.clone()),
            _ => None,
        }),
        _ => None,
    }
}

pub fn pool(home: &Home) -> Result<Vec<Vec<u8>>, Fault> {
    let ledger = home.ledger().ok().and_then(|l| l.survey().ok()).map(|s| s.items).unwrap_or_default();
    Ok(zikaron_glue::grantfile::chain_pool(crate::vaultx::held(home)?.into_iter().map(|h| h.bytes), &ledger))
}

pub fn chain_for(pool: &[Vec<u8>], id: &str) -> Result<Vec<Vec<u8>>, Fault> {
    if !zikaron::hexfmt::is_hex32(id.trim()) {
        return Err(Fault::known(Known::ContentShape, id.to_string()));
    }
    let find = |want: &str| -> Option<Entry> {
        pool.iter()
            .filter_map(|b| zikaron_glue::book::check(b).ok())
            .find(|e| e.id_hex().eq_ignore_ascii_case(want))
    };
    let mut chain: Vec<Vec<u8>> = Vec::new();
    let mut at = id.trim().to_string();
    for _ in 0..64 {
        let Some(e) = find(&at) else {
            return Err(Fault::known(Known::ChainIncomplete, at));
        };
        if e.kind != zikaron_glue::book::Kind::Grant {
            return Err(Fault::known(Known::NotAGrant, e.kind.as_str().to_string()));
        }
        let up = upstream_of(&e);
        chain.push(e.bytes.clone());
        match up {
            Some(u) => at = u,
            None => break,
        }
    }
    if chain.last().and_then(|b| zikaron_glue::book::check(b).ok()).map(|e| upstream_of(&e).is_some()).unwrap_or(false) {
        return Err(Fault::known(Known::PayloadRefused, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::TailPastHops, &[&at]))));
    }
    chain.reverse();
    Ok(chain)
}

pub fn payload_text(chain: &[Vec<u8>]) -> Result<String, Fault> {
    zikaron_kit::v2::doc::badge_encode(chain).map_err(|r| {
        Fault::known(Known::PayloadRefused, crate::lang::both_of(|| format!("{}{}", r.token.as_str(), r.index.map(|i| crate::lang::filln(crate::lang::Key::Tail087, &[&(i).to_string()])).unwrap_or_default())))
    })
}

pub fn code_for(pool: &[Vec<u8>], id: &str) -> Result<String, Fault> {
    payload_text(&chain_for(pool, id)?)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    pub ok: bool,
    pub token: String,
    pub count: Option<usize>,
    pub index: Option<usize>,
}

pub fn verify_result(payload: &[u8]) -> Checked {
    match zikaron_kit::v2::doc::badge_decode(payload) {
        Ok(v) => Checked { ok: true, token: zikaron_kit::tokens::BADGE_OK.to_string(), count: Some(v.len()), index: None },
        Err(r) => Checked { ok: false, token: r.token.as_str().to_string(), count: None, index: r.index },
    }
}

pub fn verify(payload: &[u8]) -> (bool, String) {
    let c = verify_result(payload);
    if c.ok {
        (true, format!("{} {}", c.token, c.count.unwrap_or(0)))
    } else {
        (false, format!("{}{}", c.token, c.index.map(|i| crate::lang::filln(crate::lang::Key::Tail087, &[&(i).to_string()])).unwrap_or_default()))
    }
}

pub fn chain_result(chain: &[Vec<u8>], cards: &[crate::vaultx::Card], now: Option<u64>) -> zikaron::json::Value {
    crate::trace::mark(crate::feature::Feature::D9);
    let hops: Vec<(Vec<u8>, Option<zikaron::json::Value>, Option<zikaron::json::Value>)> = chain
        .iter()
        .map(|g| {
            let id = zikaron_glue::book::check(g).map(|e| e.id_hex()).unwrap_or_default();
            let card = cards.iter().find(|c| c.id == id);
            (g.clone(), card.and_then(|c| c.input.clone()), card.and_then(|c| c.notices.clone()))
        })
        .collect();
    zikaron_glue::book::chain_check(&hops, now)
}

pub fn chain_verdict(chain: &[Vec<u8>], cards: &[crate::vaultx::Card], now: Option<u64>) -> String {
    use zikaron::json::Value;
    let v = chain_result(chain, cards, now);
    let get = |k: &str| match &v {
        Value::Obj(m) => m.iter().find(|(x, _)| x == k).map(|(_, y)| y.clone()),
        _ => None,
    };
    let verdict = match get("verdict") {
        Some(Value::Str(s)) => s,
        _ => String::new(),
    };
    let token = match get("token") {
        Some(Value::Str(s)) => s,
        _ => String::new(),
    };
    let failing = match get("failing") {
        Some(Value::Obj(m)) => {
            let kind = m.iter().find(|(k, _)| k == "kind").and_then(|(_, x)| match x { Value::Str(s) => Some(s.clone()), _ => None }).unwrap_or_default();
            let index = m.iter().find(|(k, _)| k == "index").and_then(|(_, x)| match x { Value::Int(n) => Some(*n), _ => None }).unwrap_or(0);
            format!("{kind} {index}")
        }
        _ => String::new(),
    };
    if verdict == zikaron_kit::tokens::CheckVerdict::Fail.as_str() {
        let mut s = verdict;
        if !token.is_empty() {
            s.push_str(" · ");
            s.push_str(&token);
        }
        if !failing.is_empty() {
            s.push_str(" · ");
            s.push_str(&failing);
        }
        s
    } else {
        verdict
    }
}

#[derive(Clone, Debug)]
pub struct Made {
    pub grant: String,
    pub chain: String,
    pub input_at: Option<u64>,
    pub modules: Vec<Vec<bool>>,
    pub hops: usize,
    pub payload: String,
    pub bytes: usize,
    pub verdict: String,
    pub version: usize,
    pub size: usize,
    pub txt: PathBuf,
    pub svg: PathBuf,
}

pub const TXT: &str = "badge.txt";
pub const SVG: &str = "badge.svg";

pub fn export(_pass: &crate::exitgate::Pass, chain: &[Vec<u8>], out: &Path) -> Result<Made, Fault> {
    crate::trace::mark(crate::feature::Feature::D9);
    if out.as_os_str().is_empty() {
        return Err(Fault::known(Known::FieldMissing, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail070).to_string())));
    }
    let payload = payload_text(chain)?;
    let (ok, verdict) = verify(payload.as_bytes());
    if !ok {
        return Err(Fault::known(Known::PayloadRefused, verdict));
    }
    let code = crate::qr::make(payload.as_bytes()).map_err(|e| {
        let why = match e {
            crate::qr::NotMade::TooLong => crate::lang::Key::Tail088,
            crate::qr::NotMade::SelfCheck => crate::lang::Key::Tail237,
        };
        Fault::known(Known::PayloadRefused, crate::lang::both_of(|| crate::lang::t(why).to_string()))
    })?;
    std::fs::create_dir_all(out).map_err(|e| crate::fault::classify(&e, &out.display().to_string()))?;
    let txt = out.join(TXT);
    let svg = out.join(SVG);
    for (p, b) in [(&txt, payload.as_bytes().to_vec()), (&svg, crate::qr::svg(&code).into_bytes())] {
        zikaron_glue::landing::land_bytes(p, &b).map_err(|t| {
            Fault::of_landing(t)
        })?;
    }
    Ok(Made {
        grant: String::new(),
        chain: String::new(),
        input_at: None,
        modules: code.modules.clone(),
        hops: chain.len(),
        bytes: payload.len(),
        payload,
        verdict,
        version: code.version,
        size: code.size,
        txt,
        svg,
    })
}
