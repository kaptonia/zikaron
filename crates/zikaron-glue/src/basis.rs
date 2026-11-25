use zikaron::json::Value;

pub fn senders_of(items: &[Vec<u8>]) -> Vec<String> {
    crate::book::senders_of(items)
}

pub struct Window<'a> {
    pub chain: u64,
    pub registries: &'a [String],
    pub from_block: u64,
    pub to_block: u64,
    pub senders: &'a [String],
}

pub fn basis_of(w: &Window) -> Value {
    basis_of_all(std::slice::from_ref(w))
}

pub fn basis_of_all(ws: &[Window]) -> Value {
    let mut order: Vec<&Window> = ws.iter().collect();
    order.sort_by_key(|w| (w.chain, w.from_block));
    Value::Obj(vec![("chains".to_string(), Value::Arr(order.into_iter().map(window_of).collect()))])
}

fn window_of(w: &Window) -> Value {
    Value::Obj(vec![
        ("chainId".to_string(), Value::Int(w.chain)),
        ("fromBlock".to_string(), Value::Int(w.from_block)),
        ("registries".to_string(), Value::Arr(w.registries.iter().map(|r| Value::Str(r.clone())).collect())),
        ("senders".to_string(), Value::Arr(w.senders.iter().map(|s| Value::Str(s.clone())).collect())),
        ("toBlock".to_string(), Value::Int(w.to_block)),
    ])
}
