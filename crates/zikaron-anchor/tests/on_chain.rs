//! `send::on_chain`: which hashes a sender already anchored through a registry, read from the registry's logs and
//! judged again here (the node's filtering is not trusted); a node that does not say is said in words. In-process
//! endpoints; never the network.

use zikaron::json::Value;
use zikaron_anchor::rpc::{Endpoint, Trouble};
use zikaron_anchor::send;
use zikaron_anchor::wire::{self, W};

const REG: [u8; 20] = [0x11; 20];
const ME: [u8; 20] = [0x22; 20];

fn h(b: u8) -> [u8; 32] {
    [b; 32]
}

fn hex(b: &[u8]) -> String {
    zikaron::hexfmt::encode(b)
}

fn padded(a: &[u8; 20]) -> [u8; 32] {
    let mut o = [0u8; 32];
    o[12..].copy_from_slice(a);
    o
}

fn log(address: &[u8; 20], t0: [u8; 32], sender: &[u8; 20], hash: [u8; 32], tx: u8, block: u64) -> String {
    format!(
        r#"{{"address":"{}","topics":["{}","{}","{}"],"data":"0x","blockNumber":"0x{block:x}","transactionHash":"{}","logIndex":"0x0"}}"#,
        hex(address),
        hex(&t0),
        hex(&padded(sender)),
        hex(&hash),
        hex(&[tx; 32])
    )
}

/// A node with a head and a fixed answer to the log question (`None`: it refuses it).
struct Node {
    head: Option<&'static str>,
    logs: Option<String>,
}

impl Endpoint for Node {
    fn call(&mut self, method: &str, _: &Value) -> Result<W, Trouble> {
        let s = match (method, self.head, &self.logs) {
            ("eth_blockNumber", Some(h), _) => format!("\"{h}\""),
            ("eth_getLogs", _, Some(l)) => l.clone(),
            _ => return Err(Trouble::Node("refused".into())),
        };
        Ok(wire::parse(s.as_bytes()).expect("a wire value"))
    }
    fn name(&self) -> String {
        "node".into()
    }
    fn place(&self) -> String {
        self.name()
    }
}

/// Each hash once at its earliest anchor from this sender through this registry; logs of another sender, another
/// contract or another event are not this key's anchors whatever the node returned.
#[test]
fn only_this_senders_anchors_through_this_registry_count_each_at_its_earliest() {
    let t0 = zikaron_anchor::scan::topic0();
    let logs = format!(
        "[{},{},{},{},{}]",
        log(&REG, t0, &ME, h(0xa1), 0x01, 0x20),
        log(&REG, t0, &ME, h(0xa1), 0x02, 0x10),
        log(&REG, t0, &[0x33; 20], h(0xb2), 0x03, 0x11),
        log(&[0x44; 20], t0, &ME, h(0xb2), 0x04, 0x12),
        log(&REG, h(0x99), &ME, h(0xb2), 0x05, 0x13),
    );
    let mut node = Node { head: Some("0x64"), logs: Some(logs) };
    let found = send::on_chain(&mut node, 1, &REG, &ME, &[h(0xa1), h(0xb2), h(0xc3)], 0).expect("answered");
    assert_eq!(found, vec![send::OnChain { hash: h(0xa1), tx: [0x02; 32], block: 0x10 }]);
    // Nothing asked of nothing.
    assert_eq!(send::on_chain(&mut node, 1, &REG, &ME, &[], 0), Ok(Vec::new()));
}

/// A head that does not read, or a log question refused at every range, is no answer, never "not on chain", and
/// says which question and what the node met, as structure.
#[test]
fn a_node_that_does_not_say_is_no_answer() {
    let mut no_head = Node { head: None, logs: Some("[]".into()) };
    assert!(matches!(send::on_chain(&mut no_head, 1, &REG, &ME, &[h(0xa1)], 0), Err(send::NotSaid::Head(_))));
    let mut no_logs = Node { head: Some("0x64"), logs: None };
    assert!(matches!(send::on_chain(&mut no_logs, 1, &REG, &ME, &[h(0xa1)], 0), Err(send::NotSaid::Logs(_))));
}
