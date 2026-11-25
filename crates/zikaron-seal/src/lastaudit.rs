use zikaron::json::Value;

pub const FILE: &str = "last-audit.json";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Anchored {
    pub rows: Vec<(String, (u64, String))>,
    pub at: u64,
    pub whole: bool,
}

pub fn bytes(a: &Anchored) -> Vec<u8> {
    let arr: Vec<Value> = a
        .rows
        .iter()
        .map(|(id, (chain, tx))| {
            Value::Obj(vec![
                ("chain".to_string(), Value::Int(*chain)),
                ("id".to_string(), Value::Str(id.clone())),
                ("tx".to_string(), Value::Str(tx.clone())),
            ])
        })
        .collect();
    zikaron::json::canon_bytes(&Value::Obj(vec![
        ("anchored".to_string(), Value::Arr(arr)),
        ("at".to_string(), Value::Int(a.at)),
        ("whole".to_string(), Value::Bool(a.whole)),
    ]))
}

pub fn parse(plain: &[u8]) -> Result<Anchored, String> {
    let v = zikaron::json::parse(plain).map_err(|t| format!("{t:?}"))?;
    let (Some(Value::Arr(rows)), Some(Value::Int(at))) = (v.member("anchored"), v.member("at")) else {
        return Err("shape".to_string());
    };
    let mut out = Vec::new();
    for r in rows {
        match (r.member("id"), r.member("chain"), r.member("tx")) {
            (Some(Value::Str(id)), Some(Value::Int(chain)), Some(Value::Str(tx))) => out.push((id.clone(), (*chain, tx.clone()))),
            _ => return Err("row".to_string()),
        }
    }
    let Some(Value::Bool(whole)) = v.member("whole") else { return Err("whole".to_string()) };
    Ok(Anchored { rows: out, at: *at, whole: *whole })
}
