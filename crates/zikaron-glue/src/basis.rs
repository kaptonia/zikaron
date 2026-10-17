//! A ledger's basis declaration (law §9.4): which chain, which registry contract, which blocks and which senders a
//! scan asks about. Built only here; both the desktop's self-audit and the command line's `scan` of a home use it.

use zikaron::json::Value;

/// The senders this ledger declares: the author of every entry in the pile, plus every succession's `to`.
///
/// This is a superset: the audit discards anchor records outside the whole set's lineage and lists them as
/// DISCARDED, so declaring wider only scans more, never counts more. Sorted and deduplicated (one scan, one
/// assembly).
pub fn senders_of(items: &[Vec<u8>]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for b in items {
        let Ok(e) = zikaron::entry::check(b) else { continue };
        out.push(e.author.clone());
        if e.kind == zikaron::tokens::EntryType::Succession {
            if let Some(Value::Str(to)) = e.body.member("to") {
                out.push(to.clone());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// One chain window of a basis: the chain, the registry (hex20), the blocks from and to, the senders.
pub struct Window<'a> {
    pub chain: u64,
    pub registry: &'a str,
    pub from_block: u64,
    pub to_block: u64,
    pub senders: &'a [String],
}

/// Lays out a basis of one window: `chains` holds that window, `adoptionChains` and `bareTx` are empty; members
/// in canonical byte order.
pub fn basis_of(w: &Window) -> Value {
    let window = Value::Obj(vec![
        ("chainId".to_string(), Value::Int(w.chain)),
        ("fromBlock".to_string(), Value::Int(w.from_block)),
        ("registries".to_string(), Value::Arr(vec![Value::Str(w.registry.to_string())])),
        ("senders".to_string(), Value::Arr(w.senders.iter().map(|s| Value::Str(s.clone())).collect())),
        ("toBlock".to_string(), Value::Int(w.to_block)),
    ]);
    Value::Obj(vec![
        ("adoptionChains".to_string(), Value::Arr(Vec::new())),
        ("bareTx".to_string(), Value::Arr(Vec::new())),
        ("chains".to_string(), Value::Arr(vec![window])),
    ])
}
