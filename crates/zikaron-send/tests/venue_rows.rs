use zikaron::json::Value;
use zikaron_send::node::{Endpoint, NodeAddr};
use zikaron_send::Code;

const REVOCATION_CODE: &str = include_str!("data/ZikaronRevocation.runtime.hex");
const BOOK_CODE: &str = include_str!("data/SuccessionBook.runtime.hex");

struct Anvil(std::process::Child, String);

impl Drop for Anvil {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn call(url: &str, method: &str, params: Value) -> Option<String> {
    let mut ep = zikaron_send::node::endpoint_at(url)?;
    ep.call(method, &params).ok().map(|w| zikaron_anchor::wire::write(&w))
}

fn anvil(chain: u64, number: u64) -> Option<Anvil> {
    if !std::process::Command::new("anvil").arg("--version").output().is_ok_and(|o| o.status.success()) {
        return None;
    }
    let port = std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).expect("a free port").port();
    let child = std::process::Command::new("anvil")
        .args(["--port", &port.to_string(), "--chain-id", &chain.to_string(), "--number", &number.to_string(), "--silent"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("anvil starts");
    let a = Anvil(child, format!("http://127.0.0.1:{port}"));
    for _ in 0..400 {
        if call(&a.1, "web3_clientVersion", Value::Arr(Vec::new())).is_some_and(|v| v.starts_with("\"anvil")) {
            return Some(a);
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("anvil never answered");
}

fn set_code(url: &str, at: &[u8; 20], code: &str) {
    call(url, "anvil_setCode", Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(at)), Value::Str(code.trim().to_string())])).expect("code laid");
}

#[test]
fn a_seat_on_the_three_chains_is_read_and_passes_with_its_rows_member() {
    for chain in [1u64, 42_161, 10] {
        let v = zikaron_anchor::revoke::venue(chain).expect("the chain's venue");
        let Some(a) = anvil(chain, v.from_block.max(v.book_from_block) + 100) else { return };
        let eps = vec![Endpoint::at(chain, NodeAddr::new(a.1.as_str()))];
        let root = zikaron::cryptox::address_of_privkey(&[0x42; 32]).expect("a key");
        let seat = zikaron_send::issue::Seat { root, office: root, lineage: vec![root] };
        let refused = zikaron_send::issue::check(&eps, chain, &seat).unwrap_err();
        assert_eq!(refused.code, Code::NoticeContract, "chain {chain}: no code at the row's address, refused as that contract: {refused:?}");
        set_code(&a.1, &v.contract, REVOCATION_CODE);
        set_code(&a.1, &v.book, BOOK_CODE);
        let m = zikaron_send::issue::check(&eps, chain, &seat).unwrap_or_else(|t| panic!("chain {chain}: {t:?}"));
        assert_eq!((m.chain, m.at), (chain, v.contract), "chain {chain}: the member names the row's contract");
        assert_eq!(zikaron::hexfmt::encode(&m.at), "0x0af771f809358fcc2afd13bdff98fb7cd6c9fe42");
    }
}
