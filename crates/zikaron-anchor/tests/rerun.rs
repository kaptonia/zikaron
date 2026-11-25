use std::sync::{Arc, Mutex};
use zikaron::json::Value;
use zikaron_anchor::rpc::{Endpoint, Trouble};
use zikaron_anchor::send::{self, Earlier, NotSent, Unheld};
use zikaron_anchor::wire::{self, W};

fn w(s: &str) -> W {
    wire::parse(s.as_bytes()).expect("a wire value")
}

fn quick() {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
}

struct Scripted {
    asked: Arc<Mutex<Vec<String>>>,
    answer: fn(&str, usize) -> Result<W, Trouble>,
}

impl Scripted {
    fn new(answer: fn(&str, usize) -> Result<W, Trouble>) -> (Scripted, Arc<Mutex<Vec<String>>>) {
        let asked: Arc<Mutex<Vec<String>>> = Arc::default();
        (Scripted { asked: asked.clone(), answer }, asked)
    }
}

impl Endpoint for Scripted {
    fn call(&mut self, method: &str, _: &Value) -> Result<W, Trouble> {
        let mut a = self.asked.lock().unwrap();
        let before = a.iter().filter(|m| *m == method).count();
        a.push(method.to_string());
        drop(a);
        (self.answer)(method, before)
    }
    fn name(&self) -> String {
        "scripted".into()
    }
    fn place(&self) -> String {
        self.name()
    }
}

fn count(asked: &Arc<Mutex<Vec<String>>>, method: &str) -> usize {
    asked.lock().unwrap().iter().filter(|m| *m == method).count()
}

const FROM: [u8; 20] = [0x11; 20];
const ONE: [u8; 32] = [0xa1; 32];
const TWO: [u8; 32] = [0xa2; 32];

#[test]
fn transactions_no_node_holds_are_judged_by_the_nonce_on_chain() {
    assert_eq!(send::unheld(None, 5, || true), Unheld::Wait, "not read");
    assert_eq!(send::unheld(Some(5), 5, || panic!("not asked")), Unheld::Unused, "at the sent nonce");
    assert_eq!(send::unheld(Some(0), 5, || panic!("not asked")), Unheld::Unused, "below it");
    assert_eq!(send::unheld(Some(6), 5, || true), Unheld::Void, "past it, none included");
    assert_eq!(send::unheld(Some(u64::MAX), 5, || false), Unheld::Wait, "past it, a receipt heard");
    assert_eq!(send::unheld(Some(1), 0, || true), Unheld::Void, "nonce zero used");
}

fn receipt_of_two(m: &str, _: usize) -> Result<W, Trouble> {
    Ok(match m {
        "eth_getTransactionReceipt" => w("{\"blockNumber\":\"0x9\",\"status\":\"0x1\"}"),
        _ => w("null"),
    })
}
fn held(m: &str, _: usize) -> Result<W, Trouble> {
    Ok(match m {
        "eth_getTransactionByHash" => w(&format!("{{\"hash\":\"0x{}\",\"nonce\":\"0x3\"}}", "a2".repeat(32))),
        _ => w("null"),
    })
}
fn past(m: &str, _: usize) -> Result<W, Trouble> {
    Ok(match m {
        "eth_getTransactionCount" => w("\"0x4\""),
        _ => w("null"),
    })
}
fn unused(m: &str, _: usize) -> Result<W, Trouble> {
    Ok(match m {
        "eth_getTransactionCount" => w("\"0x3\""),
        _ => w("null"),
    })
}
fn late_receipt(m: &str, before: usize) -> Result<W, Trouble> {
    Ok(match m {
        "eth_getTransactionCount" => w("\"0x4\""),
        "eth_getTransactionReceipt" if before >= 2 => w("{\"blockNumber\":\"0xa\",\"status\":\"0x0\"}"),
        _ => w("null"),
    })
}
fn receipt_shape(m: &str, _: usize) -> Result<W, Trouble> {
    Ok(match m {
        "eth_getTransactionReceipt" => w("\"0x1\""),
        _ => w("null"),
    })
}
fn held_shape(m: &str, _: usize) -> Result<W, Trouble> {
    Ok(match m {
        "eth_getTransactionByHash" => w("7"),
        _ => w("null"),
    })
}
fn nonce_shape(m: &str, _: usize) -> Result<W, Trouble> {
    Ok(match m {
        "eth_getTransactionCount" => w("\"0xzz\""),
        _ => w("null"),
    })
}
fn silent(_: &str, _: usize) -> Result<W, Trouble> {
    Err(Trouble::Transport(zikaron_anchor::rpc::Broke::Said("connection refused".into())))
}

#[test]
fn what_was_sent_before_is_asked_about_by_its_hashes() {
    quick();
    let sent = [(3u64, ONE), (3u64, TWO)];
    let ask = |answer: fn(&str, usize) -> Result<W, Trouble>| {
        let (mut ep, asked) = Scripted::new(answer);
        (send::earlier(&mut ep, &FROM, &sent), asked)
    };
    let (got, _) = ask(receipt_of_two);
    assert_eq!(got, Earlier::Included { tx: ONE, status: 1, block_number: 9 }, "in the block");
    let (got, asked) = ask(held);
    assert_eq!(got, Earlier::Held { tx: ONE }, "in the pool");
    assert_eq!(count(&asked, "eth_getTransactionCount"), 0, "in the pool: the nonce never asked");
    let (got, asked) = ask(past);
    assert_eq!(got, Earlier::Void, "held by none, nonce past");
    assert_eq!(count(&asked, "eth_getTransactionReceipt"), 4, "each receipt asked again before void");
    let (got, _) = ask(unused);
    assert_eq!(got, Earlier::Unused { nonce: 3 }, "held by none, nonce unused");
    let (got, _) = ask(late_receipt);
    assert_eq!(got, Earlier::Included { tx: ONE, status: 0, block_number: 10 }, "a receipt heard on asking again");
    for (form, answer) in [("receiptShape", receipt_shape as fn(&str, usize) -> Result<W, Trouble>), ("heldShape", held_shape), ("nonceShape", nonce_shape), ("silent", silent)] {
        let (got, _) = ask(answer);
        assert!(matches!(got, Earlier::Unread { tx, .. } if tx == TWO), "{form}: {got:?}");
    }
}

fn sends(m: &str, _: usize) -> Result<W, Trouble> {
    Ok(match m {
        "eth_blockNumber" => w("\"0x40\""),
        "eth_getBlockByNumber" => w("{\"baseFeePerGas\":\"0x3b9aca00\",\"number\":\"0x40\"}"),
        "eth_feeHistory" => w("{\"reward\":[[\"0x5f5e100\"]]}"),
        "eth_estimateGas" => w("\"0x7530\""),
        "eth_getTransactionCount" => w("\"0x7\""),
        "eth_sendRawTransaction" => w(&format!("\"0x{}\"", "00".repeat(32))),
        _ => w("null"),
    })
}
fn refuses_the_broadcast(m: &str, n: usize) -> Result<W, Trouble> {
    match m {
        "eth_sendRawTransaction" => Err(Trouble::Node("{\"code\":-32000,\"message\":\"insufficient funds for gas * price + value\"}".into())),
        other => sends(other, n),
    }
}

#[test]
fn a_send_lands_its_hash_before_the_broadcast() {
    quick();
    let key = [7u8; 32];
    let go = |answer: fn(&str, usize) -> Result<W, Trouble>, nonce: Option<u64>, land_ok: bool| {
        let (mut ep, asked) = Scripted::new(answer);
        let mut landed: Vec<(u64, [u8; 32])> = Vec::new();
        let mut land = |n: u64, h: &[u8; 32]| {
            if !land_ok {
                return Err("the disk is full".to_string());
            }
            landed.push((n, *h));
            Ok(())
        };
        let got = send::anchor_landed(&mut ep, &key, 31337, send::Form::Registry, Some([0x11; 20]), &[[0xaa; 32]], None, std::time::Duration::ZERO, nonce, &mut land, &mut |_, _, _| Ok(()));
        (got.map(|(s, _)| s.tx), landed, asked)
    };
    let (got, landed, asked) = go(sends, None, false);
    assert!(matches!(got, Err(NotSent::Land(ref w)) if w == "the disk is full"), "{got:?}");
    assert!(landed.is_empty());
    assert_eq!(count(&asked, "eth_sendRawTransaction"), 0, "a hash not landed: nothing broadcast");
    let (got, landed, asked) = go(refuses_the_broadcast, None, true);
    assert_eq!(landed.len(), 1, "landed before the broadcast");
    assert_eq!(landed[0].0, 7, "at the node's pending nonce");
    match got {
        Err(NotSent::Broadcast(tx, Trouble::Node(_))) => assert_eq!(tx, landed[0].1, "the answer carries the landed hash"),
        other => panic!("{other:?}"),
    }
    assert_eq!(count(&asked, "eth_sendRawTransaction"), 1, "broadcast once");
    let (_, landed, asked) = go(sends, Some(3), true);
    assert_eq!((landed.len(), landed.first().map(|l| l.0)), (1, Some(3)), "the nonce given is the one signed at");
    assert_eq!(count(&asked, "eth_getTransactionCount"), 0, "the node's nonce never asked");
}

fn code_limited_once(m: &str, before: usize) -> Result<W, Trouble> {
    match (m, before) {
        ("eth_getCode", 0) => Err(Trouble::Node("{\"code\":-32005,\"message\":\"rate limit exceeded\"}".into())),
        _ => Ok(w("\"0x6001\"")),
    }
}
fn code_limited_throughout(_: &str, _: usize) -> Result<W, Trouble> {
    Err(Trouble::Node("{\"code\":-32005,\"message\":\"rate limit exceeded\"}".into()))
}
fn code_refused(_: &str, _: usize) -> Result<W, Trouble> {
    Err(Trouble::Node("{\"code\":-32601,\"message\":\"the method eth_getCode does not exist\"}".into()))
}

#[test]
fn the_scans_code_question_waits_out_a_rate_limit() {
    quick();
    let pauses = zikaron_anchor::patience::RATE_LIMITED.len();
    let at = |answer: fn(&str, usize) -> Result<W, Trouble>| {
        let (mut ep, asked) = Scripted::new(answer);
        (zikaron_anchor::scan::code_at(&mut ep, 31337, &FROM, 9).map_err(|r| r.code()), count(&asked, "eth_getCode"))
    };
    assert_eq!(at(code_limited_once), (Ok(Some(true)), 2), "limited once");
    assert_eq!(at(code_limited_throughout), (Ok(None), 1 + pauses), "limited to the end");
    assert_eq!(at(code_refused), (Ok(None), 1), "another refusal");
}
