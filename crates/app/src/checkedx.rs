//! Cache of chain facts already checked. For each anchor log with a decided verdict that several endpoints
//! read alike, it keeps the block time and the sender's verdict at that block, plus the log as it was then
//! (block number, sender, hash, emitting registry), keyed by chain, block hash, transaction and log index
//! (`zikaron_anchor::scan::Known`).
//!
//! A scan still fetches every window's logs in full and judges each one. Only the four follow-up questions
//! about a log (its transaction, receipt, block time, and the sender's code at that block) are skipped, and
//! only while the log still matches what was recorded. A replaced block has a new hash, so its logs are asked
//! about as new. The cache never stands in for the chain; it only avoids asking twice what several endpoints
//! already agreed on.
//!
//! Only agreed, decided facts are stored (by `auditx`, after the endpoints agreed and more than one answered
//! for the chain); a single-endpoint reading or an unproven verdict never is. The file is sealed like every
//! local file ([`crate::local::Doc::Checked`]) under a name that identifies nobody, and is never mirrored,
//! backed up or exported. [`forget`] ("check everything again") drops it.

use crate::fault::Fault;
use zikaron::json::{self, Value};
use zikaron_anchor::scan::{Fact, FactKey, Known};

/// Directory (under the machine directory) and file name of the record.
pub const DIR: &str = "checked";
pub const FILE: &str = "facts.json";
/// Format tag stored in the record.
pub const FORM: &str = "zikaron.checked-facts/1";

fn place() -> Result<std::path::PathBuf, Fault> {
    Ok(crate::home::machine_dir()?.join(DIR).join(FILE))
}

/// The facts on record. A record that cannot be read (store locked, missing, damaged) yields no facts, so
/// every log is asked about.
pub fn read() -> Known {
    place().ok().and_then(|p| crate::local::read(&p, crate::local::Doc::Checked).ok().flatten()).and_then(|b| parse(&b)).unwrap_or_default()
}

/// Serializes [`add`] (read, merge, write) with [`forget`], so a drop that lands between an add's read and
/// write is not overwritten.
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

/// "Check everything again": deletes the record so the next scan asks about every log. Returns whether a
/// record existed.
pub fn forget() -> Result<bool, Fault> {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let p = place()?;
    match std::fs::remove_file(&p) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(crate::fault::classify(&e, &p.display().to_string())),
    }
}

/// Number of facts on record (shown in settings).
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

/// Parses the record. Any unreadable part rejects the whole record (everything is asked afresh); never
/// returns a partial table.
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

/// The facts several runs of one agreed scan read alike: every run that read the fact's chain saw the same
/// value, and more than one endpoint answered that chain (`thin` lists chains only one endpoint answered).
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

    /// A fact lands only if every run that read its chain agrees and more than one endpoint answered.
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
