//! Boundary cases: receipts that differ by block are no inclusion; a question asked of every node keeps each
//! node's name, including a node whose worker panicked; `zka`'s `--endpoint` misuse is reported in its own
//! words with exit 2. In-process endpoints and the built `zka` binary; never the network.

use std::process::Command;
use std::time::Duration;
use zikaron::json::Value;
use zikaron_anchor::endpoints;
use zikaron_anchor::rpc::{Endpoint, Trouble};
use zikaron_anchor::send::{self, Confirm};
use zikaron_anchor::wire::{self, W};

fn w(s: &str) -> W {
    wire::parse(s.as_bytes()).expect("a wire value")
}

/// A node answering every question with `answer`, under `name`.
struct Says(&'static str, &'static str);

impl Endpoint for Says {
    fn call(&mut self, _: &str, _: &Value) -> Result<W, Trouble> {
        Ok(w(self.1))
    }
    fn name(&self) -> String {
        self.0.into()
    }
    /// No address: the place is the name.
    fn place(&self) -> String {
        self.name()
    }
}

/// A node whose asking panics (a worker that falls).
struct Falls(&'static str);

impl Endpoint for Falls {
    fn call(&mut self, _: &str, _: &Value) -> Result<W, Trouble> {
        panic!("this node's worker fell")
    }
    fn name(&self) -> String {
        self.0.into()
    }
    /// No address: the place is the name.
    fn place(&self) -> String {
        self.name()
    }
}

/// `send::confirm_each`: two nodes both give a receipt, one at block 0x5 and one at 0x6 (alike in status),
/// with a wait of zero: receipts that differ are no reading, so this is not `Included`.
#[test]
fn receipts_that_differ_by_block_are_not_included() {
    let (mut a, mut b) = (
        Says("http://five", "{\"status\":\"0x1\",\"blockNumber\":\"0x5\",\"logs\":[]}"),
        Says("http://six", "{\"status\":\"0x1\",\"blockNumber\":\"0x6\",\"logs\":[]}"),
    );
    let mut eps: Vec<&mut dyn Endpoint> = vec![&mut a, &mut b];
    let got = send::confirm_each(&mut eps, &[0xab; 32], Duration::ZERO, &[]);
    assert!(!matches!(got, Confirm::Included { .. }), "{got:?}");
}

/// `endpoints::ask_each`: a node whose worker panics is reported under its own name, in its place, with a
/// transport trouble; the other nodes' answers stand.
#[test]
fn a_node_whose_worker_panics_keeps_its_name() {
    let nodes: Vec<(String, Box<dyn Endpoint + Send>)> = vec![
        ("http://one".into(), Box::new(Says("http://one", "\"0x1\""))),
        ("http://falls".into(), Box::new(Falls("http://falls"))),
        ("http://three".into(), Box::new(Says("http://three", "\"0x1\""))),
    ];
    let got = endpoints::ask_each(nodes, "eth_chainId", &Value::Arr(Vec::new()));
    let names: Vec<&str> = got.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, vec!["http://one", "http://falls", "http://three"], "every node named, in order");
    assert!(matches!(got[1].1, Err(Trouble::Transport(_))), "{:?}", got[1].1);
    assert!(got[0].1.is_ok() && got[2].1.is_ok());
}

const ZKA: &str = env!("CARGO_BIN_EXE_zka");

/// `zka`'s `--endpoint` misuse, before any node is asked: an empty address (`31337=`) is said as the flag's
/// shape, a chain id that is not a whole number (`x=http://h`) as that; each exit 2, nothing on stdout.
#[test]
fn zka_endpoint_misuse_is_said_in_its_own_words() {
    for (spec, said) in [("31337=", "zka: --endpoint 的形是 <链号>=<url>"), ("x=http://h", "zka: 链号不是十进制整数")] {
        let o = Command::new(ZKA)
            .args(["kit-capture", "--endpoint", spec, "--tx", &format!("0x{}", "11".repeat(32)), "--hash", &format!("0x{}", "22".repeat(32))])
            .output()
            .expect("zka runs");
        let err = String::from_utf8_lossy(&o.stderr);
        assert_eq!((o.status.code(), o.stdout.is_empty(), err.trim_end()), (Some(2), true, said), "{spec}");
    }
}
