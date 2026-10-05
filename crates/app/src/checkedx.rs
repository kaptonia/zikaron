//! Chain facts already checked: for each log judged an anchor with a decided verdict, read alike by several
//! endpoints of its chain, the block's time and the sender's verdict at that block, with the log as it was
//! then (block number, sender, hash, emitting registry), under the log's chain, block hash, transaction and
//! index (`zikaron_anchor::scan::Known`).
//!
//! A scan still asks every window's logs whole, with the window's senders, every time, and judges every log it
//! gets against its window; only the four questions about a log found here under the same key (its
//! transaction, receipt, block time and the sender's code at that block) are not asked again, and only while
//! the log says what it said then. A log whose block was replaced has another block hash and is asked about as
//! new; a log that says something else under a key held here is asked about as new; an anchor this record
//! lacks is asked about as always. So this record never stands in for the chain: it only spares asking twice what several
//! endpoints already said alike.
//!
//! Only agreed, decided facts land (`auditx`, after the endpoint rule agreed and more than one place answered
//! the chain): a reading from one endpoint, or an unproven verdict, never does. Sealed like every local file
//! (`local`, kind [`crate::local::Doc::Checked`]), under a name that says nobody; carried by no mirror, backup
//! or export. "Check everything again" ([`forget`]) drops it, and the next scan asks everything.

use crate::fault::Fault;
use zikaron::json::{self, Value};
use zikaron_anchor::scan::{Fact, FactKey, Known};

/// The record's room in the machine directory and its file there. One name, one home.
pub const DIR: &str = "checked";
pub const FILE: &str = "facts.json";
/// The record's form literal.
pub const FORM: &str = "zikaron.checked-facts/1";

fn place() -> Result<std::path::PathBuf, Fault> {
    Ok(crate::home::machine_dir()?.join(DIR).join(FILE))
}

/// The facts on record. A record that cannot be read (the store locked, no record yet, a damaged file) is no
/// facts: every log is asked about, which is what a scan did before this record existed.
pub fn read() -> Known {
    place().ok().and_then(|p| crate::local::read(&p, crate::local::Doc::Checked).ok().flatten()).and_then(|b| parse(&b)).unwrap_or_default()
}

/// Adding (read, merge, write) and dropping take turns: a drop that landed between an add's read and its write
/// would be written over with the facts it dropped, and "check everything again" would have done nothing.
static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Add agreed facts to the record; written once when it changed. Returns how many were new.
pub fn add(new: &[(FactKey, Fact)]) -> Result<usize, Fault> {
    if new.is_empty() {
        return Ok(0);
    }
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let mut known = read();
    let was = known.clone();
    for (k, f) in new {
        known.0.insert(*k, *f);
    }
    let added = known.0.len() - was.0.len();
    if known != was {
        let machine = crate::home::machine_dir()?;
        crate::local::put(&machine.join(DIR), FILE, crate::local::Doc::Checked, &json::canon_bytes(&value_of(&known)))?;
    }
    Ok(added)
}

/// "Check everything again": the record is dropped; the next scan asks about every log. Returns whether there
/// was one.
pub fn forget() -> Result<bool, Fault> {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let p = place()?;
    match std::fs::remove_file(&p) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(crate::fault::classify(&e, &p.display().to_string())),
    }
}

/// How many facts are on record (the settings face says it).
pub fn count() -> usize {
    read().0.len()
}

fn value_of(k: &Known) -> Value {
    let facts: Vec<Value> = k
        .0
        .iter()
        .map(|((chain, block, tx, index), f)| {
            Value::Obj(vec![
                ("block".into(), Value::Str(zikaron::hexfmt::encode(block))),
                ("blockNumber".into(), Value::Int(f.block_number)),
                ("chainId".into(), Value::Int(*chain)),
                ("emitter".into(), Value::Str(zikaron::hexfmt::encode(&f.emitter))),
                ("hash".into(), Value::Str(zikaron::hexfmt::encode(&f.hash))),
                ("index".into(), Value::Int(*index)),
                ("sender".into(), Value::Str(zikaron::hexfmt::encode(&f.sender))),
                ("timestamp".into(), Value::Int(f.block_timestamp)),
                ("tx".into(), Value::Str(zikaron::hexfmt::encode(tx))),
                ("verdict".into(), Value::Str(f.verdict.as_str().into())),
            ])
        })
        .collect();
    Value::Obj(vec![("facts".into(), Value::Arr(facts)), ("form".into(), Value::Str(FORM.into()))])
}

/// The record read back; any part that does not read makes the whole record none (asked afresh), never a
/// partial table.
fn parse(b: &[u8]) -> Option<Known> {
    let v = json::parse(b).ok()?;
    if v.member("form").and_then(|f| f.as_str()) != Some(FORM) {
        return None;
    }
    let Some(Value::Arr(facts)) = v.member("facts") else { return None };
    let h32 = |x: Option<&Value>| -> Option<[u8; 32]> { x.and_then(|s| s.as_str()).and_then(zikaron::hexfmt::decode).and_then(|b| b.try_into().ok()) };
    let h20 = |x: Option<&Value>| -> Option<[u8; 20]> { x.and_then(|s| s.as_str()).and_then(zikaron::hexfmt::decode).and_then(|b| b.try_into().ok()) };
    let int = |x: Option<&Value>| match x {
        Some(Value::Int(n)) => Some(*n),
        _ => None,
    };
    let mut out = Known::default();
    for f in facts {
        let verdict = f.member("verdict").and_then(|s| s.as_str()).and_then(zikaron::tokens::Verdict::parse)?;
        if verdict == zikaron::tokens::Verdict::Unproven {
            return None;
        }
        let key = (int(f.member("chainId"))?, h32(f.member("block"))?, h32(f.member("tx"))?, int(f.member("index"))?);
        out.0.insert(
            key,
            Fact {
                block_number: int(f.member("blockNumber"))?,
                sender: h20(f.member("sender"))?,
                hash: h32(f.member("hash"))?,
                emitter: h20(f.member("emitter"))?,
                block_timestamp: int(f.member("timestamp"))?,
                verdict,
            },
        );
    }
    Some(out)
}

/// The facts several runs of one agreed scan read alike: a fact every run that read its chain read afresh with
/// the same value, on a chain more than one place answered (`thin` lists the chains only one place did).
pub fn agreed(runs: &[(Vec<u64>, Vec<(FactKey, Fact)>)], thin: &[u64]) -> Vec<(FactKey, Fact)> {
    let Some((_, first)) = runs.first() else { return Vec::new() };
    first
        .iter()
        .filter(|(k, f)| {
            let chain = k.0;
            !thin.contains(&chain)
                && runs.iter().filter(|(chains, _)| chains.contains(&chain)).count() >= 2
                && runs.iter().filter(|(chains, _)| chains.contains(&chain)).all(|(_, seen)| seen.iter().any(|(k2, f2)| k2 == k && f2 == f))
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use zikaron::tokens::Verdict;

    fn fact(n: u64) -> Fact {
        Fact { block_number: 9, sender: [5; 20], hash: [6; 32], emitter: [7; 20], block_timestamp: n, verdict: Verdict::Counted }
    }

    /// A fact lands only when every run that read its chain read it alike, and the chain had more than one
    /// place answer.
    #[test]
    fn only_facts_every_run_read_alike_land() {
        let k = |c: u64, i: u64| (c, [1u8; 32], [2u8; 32], i);
        let runs = vec![
            (vec![1], vec![(k(1, 0), fact(10)), (k(1, 1), fact(11)), (k(1, 2), fact(12))]),
            (vec![1], vec![(k(1, 0), fact(10)), (k(1, 1), fact(99))]),
        ];
        assert_eq!(agreed(&runs, &[]), vec![(k(1, 0), fact(10))]);
        assert!(agreed(&runs, &[1]).is_empty(), "a chain one place answered gives no fact");
        assert!(agreed(&runs[..1], &[]).is_empty(), "one run is one source");
    }

    /// The record reads back what it wrote; an unproven verdict or a broken part makes it none.
    #[test]
    fn the_record_reads_back_whole_or_not_at_all() {
        let mut k = Known::default();
        k.0.insert((1, [3; 32], [4; 32], 7), Fact { block_number: 9, sender: [5; 20], hash: [6; 32], emitter: [7; 20], block_timestamp: 5, verdict: Verdict::Void });
        let b = json::canon_bytes(&value_of(&k));
        assert_eq!(parse(&b), Some(k));
        let t = String::from_utf8(b).unwrap();
        assert_eq!(parse(t.replace(Verdict::Void.as_str(), Verdict::Unproven.as_str()).as_bytes()), None);
        assert_eq!(parse(t.replace(FORM, "zikaron.checked-facts/2").as_bytes()), None);
    }
}
