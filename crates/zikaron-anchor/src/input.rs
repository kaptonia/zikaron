use zikaron::json::{canon_bytes, Value};

pub fn assemble(fragment: &Value, root: &str, pile: &[String]) -> Option<Value> {
    crate::seam();
    let anchors = fragment.member("anchors")?.clone();
    let basis = fragment.member("basis")?.clone();
    Some(Value::Obj(vec![
        ("anchors".into(), anchors),
        ("basis".into(), basis),
        ("pile".into(), Value::Arr(pile.iter().map(|x| Value::Str(x.clone())).collect())),
        ("root".into(), Value::Str(root.to_string())),
    ]))
}

pub fn audit(bytes: &[u8]) -> Vec<u8> {
    crate::seam();
    match zikaron::json::parse_tests_1_3(bytes) {
        Ok(v) => canon_bytes(&zikaron::v2::audit::audit(&v)),
        Err(_) => canon_bytes(&zikaron::v2::audit::no_label()),
    }
}
