use zikaron::json::Value;
use zikaron_send::node::{Endpoint, NodeAddr};
use zikaron_send::notices::{read, Unread};

const CHAIN: u64 = 11_155_111;
const KEY: [u8; 32] = [
    0x59, 0xc6, 0x99, 0x5e, 0x99, 0x8f, 0x97, 0xa5, 0xa0, 0x04, 0x49, 0x66, 0xf0, 0x94, 0x53, 0x89, 0xdc, 0x9e, 0x86, 0xda, 0xe8, 0x8c, 0x7a, 0x84,
    0x12, 0xf4, 0x60, 0x3b, 0x6b, 0x78, 0x69, 0x0d,
];
const OTHER: [u8; 32] = [
    0x5d, 0xe4, 0x11, 0x1a, 0xfa, 0x1a, 0x4b, 0x94, 0x90, 0x8f, 0x83, 0x10, 0x3e, 0xb1, 0xf1, 0x70, 0x63, 0x67, 0xc2, 0xe6, 0x8c, 0xa8, 0x70, 0xfc,
    0x3f, 0xb9, 0xa8, 0x04, 0xcd, 0xab, 0x36, 0x5a,
];
const REVOCATION_CODE: &str = include_str!("data/ZikaronRevocation.runtime.hex");
const BOOK_CODE: &str = include_str!("data/SuccessionBook.runtime.hex");

struct Anvil(std::process::Child, String);

impl Drop for Anvil {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn anvil() -> Option<Anvil> {
    if !std::process::Command::new("anvil").arg("--version").output().is_ok_and(|o| o.status.success()) {
        return None;
    }
    let port = std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).expect("a free port").port();
    let child = std::process::Command::new("anvil")
        .args(["--port", &port.to_string(), "--chain-id", &CHAIN.to_string(), "--number", "12200000", "--silent"])
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

fn call(url: &str, method: &str, params: Value) -> Option<String> {
    let mut ep = zikaron_send::node::endpoint_at(url)?;
    ep.call(method, &params).ok().map(|w| zikaron_anchor::wire::write(&w))
}

fn hex(b: &[u8]) -> String {
    zikaron::hexfmt::encode(b)
}

fn set_code(url: &str, at: &[u8; 20], code: &str) {
    call(url, "anvil_setCode", Value::Arr(vec![Value::Str(hex(at)), Value::Str(code.trim().to_string())])).expect("code laid");
}

fn send(url: &str, key: &[u8; 32], to: [u8; 20], data: &[u8]) -> [u8; 32] {
    let from = hex(&zikaron::cryptox::address_of_privkey(key).expect("key"));
    let n = call(url, "eth_getTransactionCount", Value::Arr(vec![Value::Str(from), Value::Str("pending".into())])).expect("nonce");
    let nonce = u64::from_str_radix(n.trim_matches('"').trim_start_matches("0x"), 16).expect("nonce reads");
    let fees = zikaron_anchor::send::Fees { max_fee: 50_000_000_000, priority: 1_000_000_000, gas_limit: 300_000, from_chain: false };
    let (raw, hash) = zikaron_anchor::send::sign_plain(key, CHAIN, to, 0, data, fees, nonce).expect("signed");
    let mut ep = zikaron_send::node::endpoint_at(url).expect("endpoint");
    zikaron_anchor::send::submit(ep.as_mut(), &raw, &hash).expect("sent");
    assert!(matches!(zikaron_anchor::send::confirm(ep.as_mut(), &hash, std::time::Duration::from_secs(30)), zikaron_anchor::send::Confirm::Included { status: 1, .. }));
    hash
}

fn revoke_data(entry: [u8; 32], grant: [u8; 32]) -> Vec<u8> {
    let mut d = vec![0xc2, 0x66, 0x46, 0x10];
    d.extend_from_slice(&entry);
    d.extend_from_slice(&grant);
    d
}

fn eps(url: &str) -> Vec<Endpoint> {
    vec![Endpoint::at(CHAIN, NodeAddr::new(url))]
}

fn records<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.member(k).and_then(Value::as_arr).map(|a| a.as_slice()).unwrap_or(&[])
}

#[test]
fn a_chains_notice_input_is_read_whole_or_named_as_not_read() {
    let Some(a) = anvil() else { return };
    let url = a.1.clone();
    let venue = zikaron_anchor::revoke::venue(CHAIN).expect("the chain's venue");
    assert_eq!(read(&eps(&url), 31_337), Err(Unread::NoVenue), "a chain with no venue");
    assert_eq!(read(&[Endpoint::at(31_337, NodeAddr::new(url.as_str()))], CHAIN), Err(Unread::NoNode), "no node of the chain");
    assert!(matches!(read(&eps(&url), CHAIN), Err(Unread::Contract(_))), "no code at the revocation contract's address");
    set_code(&url, &venue.contract, "0x6001600055");
    assert!(matches!(read(&eps(&url), CHAIN), Err(Unread::Contract(_))), "other code at the revocation contract's address");
    set_code(&url, &venue.contract, REVOCATION_CODE);
    assert_eq!(read(&eps(&url), CHAIN), Err(Unread::Book), "no book at the book's address");
    set_code(&url, &venue.book, BOOK_CODE);
    let silent = read(&eps(&url), CHAIN).expect("read");
    assert!(records(&silent.value, "revocations").is_empty() && records(&silent.value, "accessions").is_empty());
    let basis = silent.value.member("basis").expect("basis");
    assert_eq!(basis.member("fromBlock").and_then(Value::as_int), Some(venue.from_block));
    assert_eq!(basis.member("bookFromBlock").and_then(Value::as_int), Some(venue.book_from_block));
    assert_eq!(basis.member("revocationContract").and_then(Value::as_str), Some(hex(&venue.contract).as_str()));

    let (entry, grant) = ([0x0e; 32], [0x0a; 32]);
    let by_other = send(&url, &OTHER, venue.contract, &revoke_data(entry, grant));
    let by_key = send(&url, &KEY, venue.contract, &revoke_data(entry, grant));
    let key_addr = zikaron::cryptox::address_of_privkey(&KEY).unwrap();
    let other_addr = zikaron::cryptox::address_of_privkey(&OTHER).unwrap();
    let book_call = |c: zikaron_anchor::book::Call, who: &[u8; 20]| c.data(who);
    send(&url, &KEY, venue.book, &book_call(zikaron_anchor::book::Call::Declare, &other_addr));
    send(&url, &OTHER, venue.book, &book_call(zikaron_anchor::book::Call::Accede, &key_addr));
    let n = read(&eps(&url), CHAIN).expect("read");
    let rs = records(&n.value, "revocations");
    assert_eq!(rs.len(), 2, "both notices recorded, whoever sent them");
    let row = |r: &Value, k: &str| r.member(k).and_then(Value::as_str).unwrap_or_default().to_string();
    assert_eq!((row(&rs[0], "tx"), row(&rs[0], "sender")), (hex(&by_other), hex(&other_addr)));
    assert_eq!((row(&rs[1], "tx"), row(&rs[1], "sender")), (hex(&by_key), hex(&key_addr)));
    assert!(rs.iter().all(|r| row(r, "grant") == hex(&grant) && row(r, "verdict") == "counted"));
    let acc = records(&n.value, "accessions");
    assert_eq!(acc.len(), 1);
    assert_eq!((row(&acc[0], "holder"), row(&acc[0], "to"), row(&acc[0], "root")), (hex(&key_addr), hex(&other_addr), hex(&key_addr)));
}
