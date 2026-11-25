use crate::fault::Fault;
use crate::verifyx::{KitFacts, RecordRow};
use std::path::{Path, PathBuf};
use zikaron::json::{self, Value};

pub const FORM: &str = "zikaron.kit-verification/1";
pub const DIR: &str = "verified";

pub fn path_of(machine: &Path, manifest_sha256: &str) -> PathBuf {
    room(machine).join(file_name(manifest_sha256))
}

fn room(machine: &Path) -> PathBuf {
    machine.join(crate::kitsindex::DIR).join(DIR)
}

fn file_name(manifest_sha256: &str) -> String {
    format!("{manifest_sha256}.json")
}

pub fn manifest_sha256(manifest: &[u8]) -> String {
    zikaron::hexfmt::encode(&zikaron::cryptox::sha256(manifest))
}

pub struct Found<'a> {
    pub manifest: &'a [u8],
    pub at: u64,
    pub kit: &'a KitFacts,
    pub records: &'a [RecordRow],
    pub fragment: Option<&'a Value>,
}

pub fn value_of(f: &Found) -> Value {
    let mut m: Vec<(String, Value)> = Vec::new();
    let ok = f.kit.ok;
    m.push(("at".to_string(), Value::Int(f.at)));
    if let (true, Some(Value::Arr(chains))) = (ok, f.fragment.and_then(|x| x.member("basis")).and_then(|b| b.member("chains"))) {
        let keep = ["chainId", "fromBlock", "toBlock"];
        let chains = chains
            .iter()
            .map(|c| match c {
                Value::Obj(cells) => Value::Obj(cells.iter().filter(|(k, _)| keep.contains(&k.as_str())).cloned().collect()),
                other => other.clone(),
            })
            .collect();
        m.push(("basis".to_string(), Value::Obj(vec![("chains".to_string(), Value::Arr(chains))])));
    }
    m.push(("form".to_string(), Value::Str(FORM.to_string())));
    if ok {
        m.push(("kit".to_string(), Value::Str(f.kit.kit_id.clone())));
    }
    let said = if ok { zikaron_kit::tokens::KIT_OK.to_string() } else { f.kit.verdict.clone() };
    m.push(("kitVerdict".to_string(), Value::Obj(vec![("verdict".to_string(), Value::Str(said))])));
    m.push(("manifestSha256".to_string(), Value::Str(manifest_sha256(f.manifest))));
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

pub fn write(machine: &Path, f: &Found) -> Result<PathBuf, Fault> {
    let bytes = json::canon_bytes(&value_of(f));
    if let Err(t) = json::parse(&bytes) {
        return Err(Fault::known(crate::fault::Known::SettingsShape, format!("{FORM}: {t:?}")));
    }
    let sha = manifest_sha256(f.manifest);
    crate::home::put_at(&room(machine), &file_name(&sha), &bytes)?;
    Ok(path_of(machine, &sha))
}
