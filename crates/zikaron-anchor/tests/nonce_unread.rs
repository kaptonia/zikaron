//! The direct send's nonce answered with something that is not a quantity: nothing is signed or broadcast, and the
//! answer comes back as it was for the caller to say (`NotSent::Nonce`), never read as nonce zero. In-process node
//! only; no network.

use zikaron::json::Value;
use zikaron_anchor::rpc::{Endpoint, Trouble};
use zikaron_anchor::send;
use zikaron_anchor::wire::{self, W};

/// A node that answers everything a send asks before the nonce, and the nonce with words.
struct WordyNonce {
    broadcast: bool,
    landed: bool,
}

impl Endpoint for WordyNonce {
    fn call(&mut self, method: &str, _params: &Value) -> Result<W, Trouble> {
        let s = match method {
            "eth_blockNumber" => "\"0x40\"",
            "eth_getBlockByNumber" => "{\"baseFeePerGas\":\"0x3b9aca00\",\"number\":\"0x40\",\"timestamp\":\"0x64\"}",
            "eth_feeHistory" => "{\"oldestBlock\":\"0x3d\",\"baseFeePerGas\":[\"0x3b9aca00\",\"0x3b9aca00\"],\"reward\":[[\"0x5f5e100\"]]}",
            "eth_estimateGas" => "\"0x186a0\"",
            "eth_getTransactionCount" => "\"not a number\"",
            "eth_sendRawTransaction" => {
                self.broadcast = true;
                "\"0x00\""
            }
            _ => "null",
        };
        Ok(wire::parse(s.as_bytes()).expect("a wire value"))
    }
    fn name(&self) -> String {
        "http://127.0.0.1:1".into()
    }
    fn place(&self) -> String {
        self.name()
    }
}

#[test]
fn a_nonce_that_is_not_a_quantity_sends_nothing_and_comes_back_as_it_was() {
    let mut ep = WordyNonce { broadcast: false, landed: false };
    let mut landed = false;
    let got = send::anchor_landed(&mut ep, &[7u8; 32], 31337, send::Form::Registry, Some([0x11; 20]), &[[1u8; 32]], None, std::time::Duration::ZERO, None, &mut |_, _| {
        landed = true;
        Ok(())
    });
    ep.landed = landed;
    assert!(matches!(&got, Err(send::NotSent::Nonce(Some(Value::Str(w)))) if w == "not a number"), "the answer as it was: {:?}", got.as_ref().err().map(|e| format!("{e:?}")));
    assert!(!ep.landed && !ep.broadcast, "nothing landed, nothing broadcast");
}
