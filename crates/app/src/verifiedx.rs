//! The kit verification result file: what a verification on the window found, left where sibling apps read it.
//!
//! GALEED Desk and ERAVON read this desk and never the chain: which chain a kit's records are anchored on,
//! and whether the kit holds, reach them only through this file. A kit verified on the window writes one,
//! `<machine directory>/kits/verified/<manifestSha256>.json` ([`path_of`]); verifying the same kit again
//! replaces it whole.
//!
//! The file speaks only of chains read. A kit that holds is written when this pass read at least one chain;
//! when it read none (the kit names a network that is neither the main network nor a read-only one, or no
//! network is configured, or none could be read) nothing is written, for there is no chain to speak of. A pass
//! that read some chains and not others is written, the ones not read named in `missed`. A kit that does not
//! hold is written whatever was read (its verdict, no `kit`, no `basis`).
//!
//! This desk never reads it back: it is an export for people and their tools, not a second home for a
//! verdict. Plain, owner-only, temporary name then replace (`home::put_at`).
//!
//! ─── Shape ───
//!
//! `{"anchors":[…],"at":s,"basis":{…}?,"core":hex32,"form":"zikaron.kit-verification/1","kit":hex32?,
//! "kitVerdict":{"subject":"…"?,"verdict":"…"},"manifestSha256":hex32,"missed":[…],"records":[…]}`, canonical
//! key order.
//! `records` is one row per record of the kit: `content` and `anchor` (`{blockNumber, blockTimestamp,
//! chainId, registry, tx}` of the first counted anchor reaching it, or null), the same reading the verify
//! page shows. `anchors` is the scan's anchor records as they are, each with the `registry` its log came
//! from. `basis` is exactly the chains this pass read; `missed` the networks it left out, each
//! `{chainId, reading, registry}` with the reading's code (`down`, `fingerprint`: [`crate::widex::Reading::code`]),
//! empty when every network was read and when the kit does not hold. While `missed` is not empty, a record no
//! read chain reaches has a null `anchor`: its anchor may be on a network not read. `core` is the release
//! digest of the core that judged.

use crate::fault::Fault;
use crate::verifyx::{KitFacts, RecordRow};
use std::path::{Path, PathBuf};
use zikaron::json::{self, Value};
use zikaron_anchor::scan::Emitters;

/// The file's form literal. One name, one home.
pub const FORM: &str = "zikaron.kit-verification/1";
/// The room under the machine directory's `kits` room that holds the result files.
pub const DIR: &str = "verified";

/// Where the result file of the kit whose manifest bytes hash to `manifest_sha256` (hex32) is.
pub fn path_of(machine: &Path, manifest_sha256: &str) -> PathBuf {
    room(machine).join(file_name(manifest_sha256))
}

fn room(machine: &Path) -> PathBuf {
    machine.join(crate::kitsindex::DIR).join(DIR)
}

fn file_name(manifest_sha256: &str) -> String {
    format!("{manifest_sha256}.json")
}

/// The manifest's sha256, `0x` and sixty-four lowercase hex digits.
pub fn manifest_sha256(manifest: &[u8]) -> String {
    zikaron::hexfmt::encode(&zikaron::cryptox::sha256(manifest))
}

/// What one verification found, for the file.
pub struct Found<'a> {
    pub manifest: &'a [u8],
    pub at: u64,
    pub kit: &'a KitFacts,
    pub records: &'a [RecordRow],
    /// The fragment the chain reading gave (`None`: no chain was read).
    pub fragment: Option<&'a Value>,
    pub emitters: &'a Emitters,
    /// The networks this pass left out.
    pub missed: &'a [crate::widex::Missed],
}

fn h32(s: &str) -> Option<[u8; 32]> {
    zikaron::hexfmt::decode(s)?.try_into().ok()
}

/// The file's value. Writing has this one source.
pub fn value_of(f: &Found) -> Value {
    let mut m: Vec<(String, Value)> = Vec::new();
    let ok = f.kit.ok;
    let mut anchors: Vec<Value> = Vec::new();
    if let (true, Some(Value::Arr(rows))) = (ok, f.fragment.and_then(|x| x.member("anchors"))) {
        for r in rows {
            let Value::Obj(mut cells) = r.clone() else { continue };
            let int = |k: &str| match r.member(k) {
                Some(Value::Int(n)) => Some(*n),
                _ => None,
            };
            let text = |k: &str| match r.member(k) {
                Some(Value::Str(s)) => Some(s.clone()),
                _ => None,
            };
            let key = (|| Some((int("chainId")?, int("blockNumber")?, h32(&text("tx")?)?, h32(&text("hash")?)?)))();
            let registry = key.and_then(|k| f.emitters.get(&k)).and_then(|s| s.iter().next()).map(|a| Value::Str(zikaron::hexfmt::encode(a)));
            cells.push(("registry".to_string(), registry.unwrap_or(Value::Null)));
            anchors.push(Value::Obj(cells));
        }
    }
    m.push(("anchors".to_string(), Value::Arr(anchors)));
    m.push(("at".to_string(), Value::Int(f.at)));
    if let (true, Some(basis)) = (ok, f.fragment.and_then(|x| x.member("basis"))) {
        m.push(("basis".to_string(), basis.clone()));
    }
    m.push(("core".to_string(), Value::Str(crate::pinned::CORE_DIGEST.to_string())));
    m.push(("form".to_string(), Value::Str(FORM.to_string())));
    if ok {
        m.push(("kit".to_string(), Value::Str(f.kit.kit_id.clone())));
    }
    let mut verdict = Vec::new();
    if !ok && !f.kit.subject.is_empty() {
        verdict.push(("subject".to_string(), Value::Str(f.kit.subject.clone())));
    }
    let said = if ok { zikaron_kit::tokens::KIT_OK.to_string() } else { f.kit.verdict.clone() };
    verdict.push(("verdict".to_string(), Value::Str(said)));
    m.push(("kitVerdict".to_string(), Value::Obj(verdict)));
    m.push(("manifestSha256".to_string(), Value::Str(manifest_sha256(f.manifest))));
    let mut missed: Vec<(u64, String, &'static str)> = if ok { f.missed.iter().map(|x| (x.chain_id, x.registry.hex(), x.reading.code())).collect() } else { Vec::new() };
    missed.sort_unstable();
    missed.dedup();
    let missed = missed
        .into_iter()
        .map(|(chain, registry, reading)| {
            Value::Obj(vec![
                ("chainId".to_string(), Value::Int(chain)),
                ("reading".to_string(), Value::Str(reading.to_string())),
                ("registry".to_string(), Value::Str(registry)),
            ])
        })
        .collect();
    m.push(("missed".to_string(), Value::Arr(missed)));
    let records: Vec<Value> = if ok {
        f.records
            .iter()
            .map(|r| {
                let anchor = match &r.first {
                    Some(a) => Value::Obj(vec![
                        ("blockNumber".to_string(), Value::Int(a.block_number)),
                        ("blockTimestamp".to_string(), Value::Int(a.block_timestamp)),
                        ("chainId".to_string(), Value::Int(a.chain_id)),
                        ("registry".to_string(), a.registry.clone().map(Value::Str).unwrap_or(Value::Null)),
                        ("tx".to_string(), Value::Str(a.tx.clone())),
                    ]),
                    None => Value::Null,
                };
                Value::Obj(vec![("anchor".to_string(), anchor), ("content".to_string(), Value::Str(r.content.clone()))])
            })
            .collect()
    } else {
        Vec::new()
    };
    m.push(("records".to_string(), Value::Arr(records)));
    Value::Obj(m)
}

/// Write the result file of one verification, replacing any earlier one of the same kit. Returns where it
/// landed. The bytes are judged by the reader before they land: bytes the canonical reader would refuse are
/// refused by name, never written.
pub fn write(machine: &Path, f: &Found) -> Result<PathBuf, Fault> {
    let bytes = json::canon_bytes(&value_of(f));
    if let Err(t) = json::parse(&bytes) {
        return Err(Fault::known(crate::fault::Known::SettingsShape, format!("{FORM}: {t:?}")));
    }
    let sha = manifest_sha256(f.manifest);
    crate::home::put_at(&room(machine), &file_name(&sha), &bytes)?;
    Ok(path_of(machine, &sha))
}
