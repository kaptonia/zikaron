//! The gas estimate and the command line's estimated send (`send::estimate_gas`, `send::anchor_estimated`).
//!
//! Covers the estimate's edge forms (zero, the ceiling and one past it, malformed quantities, network and call
//! failures) and the command line's send order on a scripted node: fees, head, estimate at that head, nonce,
//! one broadcast, receipt. An estimate refused or past the ceiling sends nothing. Estimates from several nodes
//! are judged as a quantity, the largest taken (`judge::rule_of`), and that one reading meets the ceiling.

use zikaron::json::Value;
use zikaron_anchor::judge::{self, Missing, NoReading};
use zikaron_anchor::rpc::{Endpoint, Trouble};
use zikaron_anchor::send::{self, NoGas, NotSent, GAS_LIMIT};
use zikaron_anchor::wire::{self, W};

fn w(s: &str) -> W {
    wire::parse(s.as_bytes()).expect("a wire value")
}

fn estimate(answer: Result<Value, Trouble>) -> Result<u64, NoGas<Trouble>> {
    send::estimate_gas(|| Ok(0x40), |_| answer, Value::Null, |t| matches!(t, Trouble::Transport(_)))
}

#[test]
fn an_estimate_is_read_once_by_its_shape_and_held_at_the_ceiling() {
    let q = |s: &str| estimate(Ok(Value::Str(s.into())));
    assert_eq!(q("0x0"), Ok(0), "zero is a quantity (its limit is zero too: `limit_for(0) == 0`)");
    assert_eq!(q("0x30d40"), Ok(GAS_LIMIT), "exactly the ceiling");
    assert_eq!(q("0x30d41"), Err(NoGas::OverCap(u128::from(GAS_LIMIT) + 1)), "one past it");
    assert_eq!(q(&format!("0x{}", "f".repeat(32))), Err(NoGas::OverCap(u128::MAX)), "the widest quantity read");
    assert_eq!(q("0x00010"), Ok(16), "leading zeros read");
    for bad in ["0X10", "0x", &format!("0x1{}", "0".repeat(32)), "10", "0xzz"] {
        assert_eq!(q(bad), Err(NoGas::Unreadable(bad.to_string())), "{bad}");
    }
    assert_eq!(estimate(Ok(Value::Int(16))), Err(NoGas::NotText(Value::Int(16))));
    let net = Trouble::Transport(zikaron_anchor::rpc::Broke::Said("broke".into()));
    let node = Trouble::Node("{\"code\":3,\"message\":\"execution reverted\"}".into());
    assert_eq!(estimate(Err(net.clone())), Err(NoGas::Network(net.clone())), "the estimate broke: the network's");
    assert_eq!(estimate(Err(node.clone())), Err(NoGas::Refused(node.clone())), "the estimate refused: the call's");
    let head_net = send::estimate_gas(|| Err(net.clone()), |_| Ok(Value::Str("0x1".into())), Value::Null, |t| matches!(t, Trouble::Transport(_)));
    assert_eq!(head_net, Err(NoGas::Network(net)), "the head broke: the network's, the estimate never asked");
    let head_node = send::estimate_gas(|| Err(node.clone()), |_| Ok(Value::Str("0x1".into())), Value::Null, |t| matches!(t, Trouble::Transport(_)));
    assert_eq!(head_node, Err(NoGas::Refused(node)));
    // The limit an estimate gives: one and a half times, rounded up, held at the ceiling (between 133,334 and
    // 200,000 the ceiling takes some of the half: by design).
    assert_eq!((send::limit_for(30_000), send::limit_for(133_334), send::limit_for(GAS_LIMIT)), (45_000, GAS_LIMIT, GAS_LIMIT));
}

/// Several nodes' estimates of one call, judged by the one table and then read by the one estimate rule, as the
/// app does (`chainx::ask_call` then `send::estimate_gas`). Each form is one row of the estimate's closed table:
/// estimates that differ (the two Sepolia preset nodes' answers at one block), alike, one refusing, all
/// refusing, one not a quantity, none a quantity, the largest past the ceiling, zero, the widest quantity, one
/// digit past it.
#[test]
fn estimates_that_differ_are_a_quantity_read_by_the_largest() {
    let reverted = || Trouble::Node("{\"code\":3,\"message\":\"execution reverted\"}".into());
    let judged = |list: Vec<(&str, Result<&str, Trouble>)>| {
        let answers = list.into_iter().map(|(n, a)| (n.to_string(), a.map(w))).collect();
        judge::judge("eth_estimateGas", &Value::Arr(vec![]), answers)
    };
    let read = |j: &judge::Judged| estimate(Ok(j.value.clone()));
    // Differ: no longer a disagreement; the largest is the reading, both nodes its sources.
    let j = judged(vec![("http://a", Ok("\"0x443a\"")), ("http://b", Ok("\"0x4398\""))]).expect("differing estimates are read");
    assert_eq!((read(&j), j.sources.len(), j.single_source), (Ok(0x443a), 2, false), "differ");
    let j = judged(vec![("http://a", Ok("\"0x4398\"")), ("http://b", Ok("\"0x443a\"")), ("http://c", Ok("\"0x5ce9\""))]).expect("three");
    assert_eq!(read(&j), Ok(0x5ce9), "the largest of three, wherever it stands");
    // Alike: the same reading as before.
    let j = judged(vec![("http://a", Ok("\"0x5c12\"")), ("http://b", Ok("\"0x5c12\""))]).expect("alike");
    assert_eq!((read(&j), j.single_source), (Ok(0x5c12), false), "alike");
    // One refuses: passed over and named; the other speaks alone, single-source.
    let j = judged(vec![("http://a", Err(reverted())), ("http://b", Ok("\"0x4398\""))]).expect("one answered");
    assert_eq!((read(&j), j.single_source), (Ok(0x4398), true), "one refused");
    assert_eq!(j.missing, vec![("http://a".to_string(), Missing::Trouble(reverted()))]);
    // All refuse: no reading, each named with its own trouble (the caller reads the first: the call's refusal).
    match judged(vec![("http://a", Err(reverted())), ("http://b", Err(reverted()))]) {
        Err(NoReading::NoneAnswered(m)) => assert_eq!(m.len(), 2, "all refused: {m:?}"),
        other => panic!("all refused: {other:?}"),
    }
    // One answer not a quantity: passed over by its text; the quantity is the reading.
    let j = judged(vec![("http://a", Ok("\"0xzz\"")), ("http://b", Ok("\"0x4398\""))]).expect("one read");
    assert_eq!((read(&j), j.missing.clone()), (Ok(0x4398), vec![("http://a".to_string(), Missing::Unreadable("\"0xzz\"".into()))]), "one not a quantity");
    // None a quantity (text that is not one, not text at all): no reading, each named as unreadable.
    match judged(vec![("http://a", Ok("\"0xzz\"")), ("http://b", Ok("16"))]) {
        Err(NoReading::NoneAnswered(m)) => assert!(m.len() == 2 && m.iter().all(|(_, x)| matches!(x, Missing::Unreadable(_))), "none a quantity: {m:?}"),
        other => panic!("none a quantity: {other:?}"),
    }
    // The largest past the ceiling: refused by the largest's own number, though the other is within it.
    let j = judged(vec![("http://a", Ok("\"0x30d40\"")), ("http://b", Ok("\"0x30d41\""))]).expect("read");
    assert_eq!(read(&j), Err(NoGas::OverCap(u128::from(GAS_LIMIT) + 1)), "the largest one past the ceiling");
    // Zero and the ceiling: the ceiling, which is within.
    let j = judged(vec![("http://a", Ok("\"0x0\"")), ("http://b", Ok("\"0x30d40\""))]).expect("read");
    assert_eq!(read(&j), Ok(GAS_LIMIT), "zero beside the ceiling");
    let j = judged(vec![("http://a", Ok("\"0x0\"")), ("http://b", Ok("\"0x0\""))]).expect("read");
    assert_eq!(read(&j), Ok(0), "zero alone");
    // The widest quantity read, and one digit past it (passed over, so the other is the reading).
    let widest = format!("\"0x{}\"", "f".repeat(32));
    let j = judged(vec![("http://a", Ok(widest.as_str())), ("http://b", Ok("\"0x1\""))]).expect("read");
    assert_eq!(read(&j), Err(NoGas::OverCap(u128::MAX)), "the widest quantity");
    let past = format!("\"0x1{}\"", "0".repeat(32));
    let j = judged(vec![("http://a", Ok(past.as_str())), ("http://b", Ok("\"0x1\""))]).expect("read");
    assert_eq!((read(&j), j.missing.len()), (Ok(1), 1), "one digit past the widest is not a quantity");
}

/// A node answering as one does, with the estimate the test sets; every question asked is kept, in order.
struct Scripted {
    asked: Vec<(String, String)>,
    estimate: Result<W, Trouble>,
}

impl Endpoint for Scripted {
    fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
        self.asked.push((method.to_string(), String::from_utf8_lossy(&zikaron::json::canon_bytes(params)).into_owned()));
        match method {
            "eth_blockNumber" => Ok(w("\"0x40\"")),
            "eth_getBlockByNumber" => Ok(w("{\"baseFeePerGas\":\"0x3b9aca00\",\"number\":\"0x40\",\"timestamp\":\"0x64\"}")),
            "eth_feeHistory" => Ok(w("{\"oldestBlock\":\"0x2d\",\"reward\":[[\"0x5f5e100\"]]}")),
            "eth_estimateGas" => self.estimate.clone(),
            "eth_getTransactionCount" => Ok(w("\"0x0\"")),
            "eth_sendRawTransaction" => {
                let raw = params.as_arr().and_then(|a| a.first()).and_then(|x| x.as_str()).and_then(zikaron::hexfmt::decode).unwrap_or_default();
                Ok(w(&format!("\"{}\"", zikaron::hexfmt::encode(&zikaron::cryptox::keccak256(&raw)))))
            }
            "eth_getTransactionReceipt" => Ok(w("null")),
            _ => Err(Trouble::Transport(zikaron_anchor::rpc::Broke::Said(format!("not asked: {method}")))),
        }
    }
    fn name(&self) -> String {
        "scripted".into()
    }
    /// No address: the place is the name.
    fn place(&self) -> String {
        self.name()
    }
}

fn send_with(estimate: Result<W, Trouble>) -> (Result<u64, NotSent>, Vec<(String, String)>) {
    let mut ep = Scripted { asked: Vec::new(), estimate };
    let got = send::anchor_estimated(&mut ep, &[7u8; 32], 31337, send::Form::Bare, None, &[[0xaa; 32]], None, std::time::Duration::ZERO).map(|(_, limit)| limit);
    (got, ep.asked)
}

#[test]
fn the_command_line_asks_fees_then_the_estimate_at_the_pinned_head_then_sends_once() {
    let (got, asked) = send_with(Ok(w("\"0x7530\"")));
    assert_eq!(got.ok(), Some(45_000), "the limit the estimate gives");
    let methods: Vec<&str> = asked.iter().map(|(m, _)| m.as_str()).collect();
    assert_eq!(
        methods,
        [
            "eth_blockNumber",
            "eth_getBlockByNumber",
            "eth_feeHistory",
            "eth_blockNumber",
            "eth_estimateGas",
            "eth_getTransactionCount",
            "eth_sendRawTransaction",
            "eth_getTransactionReceipt",
        ]
    );
    let (_, params) = &asked[4];
    assert!(params.ends_with(",\"0x40\"]"), "the estimate is asked at the pinned head: {params}");
}

#[test]
fn an_estimate_that_gives_no_figure_sends_nothing() {
    let reverted = Trouble::Node("{\"code\":3,\"message\":\"execution reverted\"}".into());
    let limited = Trouble::Node("{\"code\":-32005,\"message\":\"rate limit exceeded\"}".into());
    for (form, estimate) in [("refusedAboutTheCall", Err(reverted)), ("rateLimited", Err(limited)), ("pastTheCeiling", Ok(w("\"0x30d41\""))), ("notText", Ok(w("16")))] {
        let (got, asked) = send_with(estimate);
        assert!(matches!(got, Err(NotSent::Gas(_))), "{form}: {got:?}");
        assert!(!asked.iter().any(|(m, _)| m == "eth_sendRawTransaction" || m == "eth_getTransactionCount"), "{form}: nothing sent");
    }
    let (got, _) = send_with(Err(Trouble::Node("{\"code\":-32005,\"message\":\"rate limit exceeded\"}".into())));
    assert!(matches!(got, Err(NotSent::Gas(NoGas::Network(_)))), "a rate limit says nothing about the call");
    let (got, _) = send_with(Err(Trouble::Node("{\"code\":3,\"message\":\"execution reverted\"}".into())));
    assert!(matches!(got, Err(NotSent::Gas(NoGas::Refused(_)))), "a revert is the call's");
}

/// A replacement's fees: the fee rule's pair read now, each field at least ten percent above the transaction it
/// replaces (rounded up, and at least one wei more), the gas limit kept, the priority fee never above the cap.
#[test]
fn a_replacement_rises_by_the_pools_bump_at_least() {
    use zikaron_anchor::send::{Fees, REPLACE_BUMP_PERCENT};
    assert_eq!(REPLACE_BUMP_PERCENT, 10);
    let old = Fees { max_fee: 2_000_000_001, priority: 1_000_000_000, gas_limit: 90_000, from_chain: true };
    // The price now is lower than the old pair: the bump decides both fields.
    let low = Fees::of(Some(100), Some(5));
    let r = Fees::replacing(old, low);
    assert_eq!((r.max_fee, r.priority, r.gas_limit), (2_200_000_002, 1_100_000_000, 90_000));
    // The price now is higher: the rule's pair decides.
    let high = Fees::of(Some(5_000_000_000), Some(1_000_000_000));
    let r = Fees::replacing(old, high);
    assert_eq!((r.max_fee, r.priority, r.gas_limit), (11_000_000_000, 1_100_000_000, 90_000));
    assert!(r.from_chain);
    // Tiny numbers still rise (ten percent of 1 rounds up to 1 more).
    let tiny = Fees { max_fee: 1, priority: 1, gas_limit: 21_000, from_chain: true };
    let r = Fees::replacing(tiny, Fees::of(Some(0), Some(0)));
    assert_eq!((r.max_fee, r.priority), (2, 2));
    assert!(r.priority <= r.max_fee);
}
