use crate::checked::Room;
use std::collections::BTreeSet;
use zikaron_anchor::proofs::AnchorProof;
use zikaron_anchor::rpc::Endpoint;
use zikaron_anchor::scan::{Fact, FactKey, Known};

pub const PREFIX: &str = "proof-";

pub fn proof_file(key: &FactKey) -> String {
    let mut b = Vec::with_capacity(8 + 32 + 32 + 8);
    b.extend_from_slice(&key.0.to_be_bytes());
    b.extend_from_slice(&key.1);
    b.extend_from_slice(&key.2);
    b.extend_from_slice(&key.3.to_be_bytes());
    let d = zikaron::cryptox::sha256(&b);
    let hex: String = d[..16].iter().map(|x| format!("{x:02x}")).collect();
    format!("{PREFIX}{hex}.json")
}

pub fn is_proof(file: &str) -> bool {
    file.starts_with(PREFIX) && file.ends_with(".json")
}

pub fn held<R: Room + ?Sized>(room: &R, key: &FactKey) -> Option<AnchorProof> {
    let bytes = room.read(&proof_file(key))?;
    AnchorProof::of_value(&zikaron::json::parse(&bytes).ok()?)
}

pub fn own(facts: &Known, senders: &[[u8; 20]]) -> Vec<(FactKey, Fact)> {
    facts.0.iter().filter(|(_, f)| senders.contains(&f.sender)).map(|(k, f)| (*k, f.clone())).collect()
}

pub fn for_export<R: Room + ?Sized>(room: &R, senders: &[[u8; 20]], ground: &zikaron_seal::ground::Ground, open: &dyn Fn(&str) -> Option<Box<dyn Endpoint + Send>>) -> Vec<AnchorProof> {
    let nodes = crate::node::settings_nodes(ground);
    let facts = crate::checked::facts_single(room);
    let mut out = Vec::new();
    for (key, fact) in own(&facts, senders) {
        if let Some(p) = held(room, &key) {
            out.push(p);
            continue;
        }
        let built = nodes.iter().filter(|e| e.chain == key.0).find_map(|e| {
            let mut ep = open(e.url.for_transport())?;
            zikaron_anchor::proofs::build(ep.as_mut(), key.0, &key.1, &key.2, &fact.sender, &fact.hash, &fact.emitter).ok()
        });
        if let Some(p) = built {
            let _ = room.put(&proof_file(&key), &zikaron::json::canon_bytes(&p.to_value()));
            out.push(p);
        }
    }
    if !facts.0.is_empty() {
        let on_record: BTreeSet<String> = facts.0.keys().map(proof_file).collect();
        for file in room.files().into_iter().filter(|f| is_proof(f) && !on_record.contains(f)) {
            room.remove(&file);
        }
    }
    out
}

pub fn senders_from(hexes: &[String]) -> Vec<[u8; 20]> {
    hexes.iter().filter_map(|s| zikaron::hexfmt::decode(s).and_then(|b| <[u8; 20]>::try_from(b).ok())).collect()
}
