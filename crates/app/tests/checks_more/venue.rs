use super::a8_send::{first_param, method_of, node, Node, Reply};
use std::sync::{Arc, Mutex};
use zikaron::json::Value;

pub(super) const CHAIN: u64 = 11_155_111;

const REVOCATION_CODE: &str = include_str!("../../../zikaron-send/tests/data/ZikaronRevocation.runtime.hex");
const BOOK_CODE: &str = include_str!("../../../zikaron-send/tests/data/SuccessionBook.runtime.hex");

#[derive(Default)]
struct Chain {
    head: u64,
    logs: Vec<Value>,
    txs: Vec<([u8; 32], Value, Value)>,
    nonces: Vec<([u8; 20], u64)>,
}

fn hex(b: &[u8]) -> String {
    zikaron::hexfmt::encode(b)
}

fn qty(n: u64) -> String {
    format!("\"0x{n:x}\"")
}

fn w32(a: &[u8; 20]) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[12..].copy_from_slice(a);
    w
}

fn a20(s: &str) -> Option<[u8; 20]> {
    zikaron::hexfmt::decode(s)?.try_into().ok()
}

fn number(v: Option<&Value>) -> Option<u64> {
    v.and_then(Value::as_str).and_then(|x| u64::from_str_radix(x.trim_start_matches("0x"), 16).ok())
}

pub(super) fn venue() -> Node {
    let start = zikaron_anchor::book::from_block(CHAIN).expect("the book's deployment block") + 64;
    let st = Arc::new(Mutex::new(Chain { head: start, ..Default::default() }));
    node(move |method, body| answer(&st, method, body))
}

pub(super) fn in_venue(shell: &mut app::shell::Shell, n: &Node) {
    let row = zikaron_anchor::deploy::ROWS.iter().find(|r| r.chain_id == CHAIN).expect("the chain's row");
    let spec = format!("{CHAIN}={}", n.url);
    let set = |s: &mut app::settings::Settings| {
        s.chain_id = Some(CHAIN);
        s.registry = app::key::Address::parse(row.registry);
        s.from_block = row.from_block;
        s.endpoints = vec![spec.clone()];
    };
    if shell.commit_settings(&set).is_err() {
        set(&mut shell.settings);
    }
    shell.endpoints = app::chainx::nodes_of(&shell.settings);
}

fn answer(st: &Arc<Mutex<Chain>>, method: &str, body: &str) -> Reply {
    let req = zikaron::json::parse(body.as_bytes()).unwrap_or(Value::Null);
    let params = req.member("params").and_then(Value::as_arr).cloned().unwrap_or_default();
    let venue = zikaron_anchor::revoke::venue(CHAIN).expect("the venue");
    let chain = st.lock().expect("the chain");
    Reply::Result(match method {
        "eth_chainId" => qty(CHAIN),
        "eth_blockNumber" => qty(chain.head),
        "eth_getBlockByNumber" => {
            let n = number(params.first()).unwrap_or(chain.head);
            format!("{{\"baseFeePerGas\":\"0x3b9aca00\",\"hash\":\"{}\",\"number\":\"0x{n:x}\",\"timestamp\":\"0x{:x}\"}}", hex(&zikaron::cryptox::keccak256(&n.to_be_bytes())), 1_800_000_000 + n % 100_000 * 12)
        }
        "eth_feeHistory" => "{\"oldestBlock\":\"0x3e\",\"gasUsedRatio\":[0.5,0.25,0.125],\"reward\":[[\"0x5f5e100\"],[\"0x5f5e100\"],[\"0x5f5e100\"]]}".into(),
        "eth_maxPriorityFeePerGas" => "\"0x5f5e100\"".into(),
        "eth_gasPrice" => "\"0x3b9aca00\"".into(),
        "eth_estimateGas" => qty(60_000),
        "eth_getBalance" => "\"0x56bc75e2d63100000\"".into(),
        "eth_getTransactionCount" => {
            let who = a20(&first_param(body));
            qty(chain.nonces.iter().find(|(a, _)| Some(*a) == who).map(|(_, n)| *n).unwrap_or(0))
        }
        "eth_getCode" => {
            let at = a20(&first_param(body));
            let code = if at == Some(venue.contract) {
                REVOCATION_CODE.trim()
            } else if at == Some(venue.book) {
                BOOK_CODE.trim()
            } else {
                "0x"
            };
            format!("\"{code}\"")
        }
        "eth_call" => {
            let call = params.first();
            let to = call.and_then(|c| c.member("to")).and_then(Value::as_str).and_then(a20);
            let data = call.and_then(|c| c.member("data").or_else(|| c.member("input"))).and_then(Value::as_str).and_then(zikaron::hexfmt::decode).unwrap_or_default();
            if to != Some(venue.book) || data.len() != 36 {
                return Reply::Result("\"0x\"".into());
            }
            let who: [u8; 20] = data[16..36].try_into().unwrap_or([0u8; 20]);
            use zikaron_anchor::book::Call as B;
            let sel: [u8; 4] = data[..4].try_into().unwrap_or([0u8; 4]);
            let word = if sel == B::Current.selector() || sel == B::Root.selector() { w32(&who) } else { [0u8; 32] };
            format!("\"{}\"", hex(&word))
        }
        "eth_getLogs" => {
            let f = params.first();
            let addrs: Vec<String> = match f.and_then(|f| f.member("address")) {
                Some(Value::Arr(a)) => a.iter().filter_map(Value::as_str).map(str::to_ascii_lowercase).collect(),
                Some(Value::Str(a)) => vec![a.to_ascii_lowercase()],
                _ => Vec::new(),
            };
            let topics: Vec<Vec<String>> = f
                .and_then(|f| f.member("topics"))
                .and_then(Value::as_arr)
                .map(|t| {
                    t.iter()
                        .map(|x| match x {
                            Value::Str(s) => vec![s.to_ascii_lowercase()],
                            Value::Arr(a) => a.iter().filter_map(Value::as_str).map(str::to_ascii_lowercase).collect(),
                            _ => Vec::new(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let from = number(f.and_then(|f| f.member("fromBlock"))).unwrap_or(0);
            let to = number(f.and_then(|f| f.member("toBlock"))).unwrap_or(u64::MAX);
            let picked: Vec<String> = chain
                .logs
                .iter()
                .filter(|l| {
                    let a = l.member("address").and_then(Value::as_str).unwrap_or_default().to_ascii_lowercase();
                    let b = number(l.member("blockNumber")).unwrap_or(0);
                    let ts: Vec<String> = l.member("topics").and_then(Value::as_arr).map(|t| t.iter().filter_map(Value::as_str).map(str::to_ascii_lowercase).collect()).unwrap_or_default();
                    (addrs.is_empty() || addrs.contains(&a)) && b >= from && b <= to && topics.iter().enumerate().all(|(i, want)| want.is_empty() || ts.get(i).is_some_and(|t| want.contains(t)))
                })
                .map(|l| String::from_utf8_lossy(&zikaron::json::canon_bytes(l)).to_string())
                .collect();
            format!("[{}]", picked.join(","))
        }
        "eth_getTransactionByHash" | "eth_getTransactionReceipt" => {
            let h: Option<[u8; 32]> = zikaron::hexfmt::decode(&first_param(body)).and_then(|b| b.try_into().ok());
            let found = chain.txs.iter().find(|(x, _, _)| Some(*x) == h).map(|(_, t, r)| if method == "eth_getTransactionByHash" { t.clone() } else { r.clone() });
            found.map(|v| String::from_utf8_lossy(&zikaron::json::canon_bytes(&v)).to_string()).unwrap_or_else(|| "null".into())
        }
        "eth_sendRawTransaction" => {
            drop(chain);
            let raw = zikaron::hexfmt::decode(&first_param(body)).unwrap_or_default();
            mine(st, &raw);
            format!("\"{}\"", hex(&zikaron::cryptox::keccak256(&raw)))
        }
        _ => {
            let _ = method_of(body);
            "null".into()
        }
    })
}

fn nonce_of(raw: &[u8]) -> u64 {
    let items = zikaron_anchor::rlp::decode_all(raw.get(1..).unwrap_or_default()).and_then(|i| i.list().map(|x| x.to_vec())).unwrap_or_default();
    items.get(1).and_then(|x| x.bytes()).map(|b| b.iter().fold(0u64, |n, x| (n << 8) | *x as u64)).unwrap_or(0)
}

fn mine(st: &Arc<Mutex<Chain>>, raw: &[u8]) {
    let Some(tx) = zikaron_anchor::tx::from_raw(raw) else { return };
    let Some(from) = tx.sender else { return };
    let venue = zikaron_anchor::revoke::venue(CHAIN).expect("the venue");
    let mut c = st.lock().expect("the chain");
    if c.txs.iter().any(|(h, _, _)| *h == tx.hash) {
        return;
    }
    c.head += 1;
    let block = c.head;
    let next = nonce_of(raw) + 1;
    match c.nonces.iter_mut().find(|(a, _)| *a == from) {
        Some((_, n)) => *n = (*n).max(next),
        None => c.nonces.push((from, next)),
    }
    let input = &tx.input;
    let word = |i: usize| -> [u8; 32] { input.get(i..i + 32).and_then(|w| w.try_into().ok()).unwrap_or([0u8; 32]) };
    let mut topics: Vec<Vec<[u8; 32]>> = Vec::new();
    if tx.to == Some(venue.contract) && input.len() == 68 && input[..4] == zikaron_anchor::revoke::calldata(&[0; 32], &[0; 32])[..4] {
        topics.push(vec![zikaron_anchor::scan::topic0(), w32(&from), word(4)]);
        topics.push(vec![zikaron::cryptox::keccak256(zikaron_anchor::revoke::REVOKED_SIGNATURE.as_bytes()), w32(&from), word(36)]);
    }
    let bh = Value::Str(hex(&zikaron::cryptox::keccak256(&block.to_be_bytes())));
    let q = |n: u64| Value::Str(format!("0x{n:x}"));
    let base = c.logs.len();
    let logs: Vec<Value> = topics
        .into_iter()
        .enumerate()
        .map(|(k, t)| {
            Value::Obj(vec![
                ("address".into(), Value::Str(hex(&venue.contract))),
                ("blockHash".into(), bh.clone()),
                ("blockNumber".into(), q(block)),
                ("data".into(), Value::Str("0x".into())),
                ("logIndex".into(), q((base + k) as u64)),
                ("removed".into(), Value::Bool(false)),
                ("topics".into(), Value::Arr(t.iter().map(|x| Value::Str(hex(x))).collect())),
                ("transactionHash".into(), Value::Str(hex(&tx.hash))),
                ("transactionIndex".into(), q(0)),
            ])
        })
        .collect();
    c.logs.extend(logs.iter().cloned());
    let to = Value::Str(tx.to.map(|t| hex(&t)).unwrap_or_default());
    let items = zikaron_anchor::rlp::decode_all(raw.get(1..).unwrap_or_default()).and_then(|i| i.list().map(|x| x.to_vec())).unwrap_or_default();
    let field = |i: usize| items.get(i).and_then(|x| x.bytes()).map(|x| x.to_vec()).unwrap_or_default();
    let num = |i: usize| {
        let b = field(i);
        Value::Str(format!("0x{:x}", b.iter().fold(0u128, |n, x| (n << 8) | *x as u128)))
    };
    let t = Value::Obj(vec![
        ("accessList".into(), Value::Arr(Vec::new())),
        ("blockHash".into(), bh.clone()),
        ("blockNumber".into(), q(block)),
        ("chainId".into(), num(0)),
        ("from".into(), Value::Str(hex(&from))),
        ("gas".into(), num(4)),
        ("hash".into(), Value::Str(hex(&tx.hash))),
        ("input".into(), Value::Str(hex(&field(7)))),
        ("maxFeePerGas".into(), num(3)),
        ("maxPriorityFeePerGas".into(), num(2)),
        ("nonce".into(), num(1)),
        ("r".into(), Value::Str(hex(&field(10)))),
        ("s".into(), Value::Str(hex(&field(11)))),
        ("to".into(), Value::Str(hex(&field(5)))),
        ("transactionIndex".into(), q(0)),
        ("type".into(), Value::Str("0x2".into())),
        ("v".into(), num(9)),
        ("value".into(), num(6)),
        ("yParity".into(), num(9)),
    ]);
    let r = Value::Obj(vec![
        ("blockHash".into(), bh),
        ("blockNumber".into(), q(block)),
        ("effectiveGasPrice".into(), q(1_000_000_000)),
        ("from".into(), Value::Str(hex(&from))),
        ("gasUsed".into(), q(30_000)),
        ("logs".into(), Value::Arr(logs)),
        ("status".into(), q(1)),
        ("to".into(), to),
        ("transactionHash".into(), Value::Str(hex(&tx.hash))),
        ("transactionIndex".into(), q(0)),
    ]);
    c.txs.push((tx.hash, t, r));
}
