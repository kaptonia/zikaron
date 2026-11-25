use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

const BIN: &str = env!("CARGO_BIN_EXE_zikaron");
const KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";
const SECRET: &str = "S3CRETk3y";

struct Ran {
    code: i32,
    out: Vec<u8>,
    err: String,
}

fn run(args: &[&str]) -> Ran {
    let mut c = Command::new(BIN);
    c.args(args).env(zikaron_os::HOME_VAR, own_home());
    for k in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
        c.env_remove(k);
    }
    let o = c.output().expect("zikaron runs");
    Ran { code: o.status.code().unwrap_or(-1), out: o.stdout, err: String::from_utf8_lossy(&o.stderr).to_string() }
}

fn own_home() -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    std::env::temp_dir().join(format!("zk-cli-home-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)))
}

fn anchor(value: &str) -> Ran {
    let hash = format!("0x{}", "11".repeat(32));
    run(&["anchor", "--endpoint", value, "--key", KEY, "--form", "registry", "--registry", "0x5fbdb2315678afecb367f032d93f642f64180aa3", "--hash", &hash, "--wait-secs", "0"])
}

fn refused(r: &Ran, first: &str, second: &str) {
    let lines: Vec<&str> = r.err.lines().collect();
    assert_eq!((r.code, lines.as_slice()), (2, [first, second].as_slice()), "{}", r.err);
    assert!(r.out.is_empty(), "stdout: {}", String::from_utf8_lossy(&r.out));
    assert!(!r.err.contains(SECRET), "the key is echoed: {}", r.err);
}

const SHAPE: &str = "--endpoint is <chain id>=<url>";
const NOT_INT: &str = "the chain id is not a decimal integer";
const SCHEME: &str = "an endpoint is an http:// or https:// address";
const PORT: &str = "an endpoint's port must be a whole number from 0 to 65535";
const ADDRESS: &str = "not an endpoint address (no host, a bracket that is not an IPv6 address, or a user name or white space in it)";
const REST_CHAIN: &str = "a REST row's chain after #rest= is not the <chain id>= it is given for";
const REST_BASE: &str = "a REST row's Base URL is https:// with no query or fragment";

#[test]
fn a_url_without_a_chain_id_is_named_by_place_and_length() {
    let v = format!("https://node.example/v3/{SECRET}");
    refused(&anchor(&v), &format!("E_ARGS #3 ({} bytes)", v.len()), SHAPE);
}

#[test]
fn an_empty_value_is_named_by_place_and_length() {
    refused(&anchor(""), "E_ARGS #3 (0 bytes)", SHAPE);
}

#[test]
fn an_empty_chain_id_is_named_by_place_and_length() {
    let v = format!("=https://node.example/{SECRET}");
    refused(&anchor(&v), &format!("E_ARGS #3 ({} bytes)", v.len()), NOT_INT);
}

#[test]
fn a_url_with_an_equals_in_its_query_is_not_said_in_part() {
    let v = format!("https://node.example/?apikey={SECRET}");
    let r = anchor(&v);
    refused(&r, &format!("E_ARGS #3 ({} bytes)", v.len()), NOT_INT);
    assert!(!r.err.contains("node.example"), "{}", r.err);
}

#[test]
fn a_chain_id_past_64_bits_is_named_by_place_and_length() {
    let v = format!("18446744073709551616=https://node.example/{SECRET}");
    refused(&anchor(&v), &format!("E_ARGS #3 ({} bytes)", v.len()), NOT_INT);
}

#[test]
fn a_negative_chain_id_is_named_by_place_and_length() {
    let v = format!("-1=https://node.example/{SECRET}");
    refused(&anchor(&v), &format!("E_ARGS #3 ({} bytes)", v.len()), NOT_INT);
}

#[test]
fn a_chain_id_with_no_address_says_the_chain_id() {
    refused(&anchor("31337="), "E_ARGS #3 (31337= + 0 bytes)", SHAPE);
}

#[test]
fn a_chain_id_with_only_white_space_after_says_the_chain_id_and_that_length() {
    refused(&anchor("31337=   "), "E_ARGS #3 (31337= + 3 bytes)", SHAPE);
}

#[test]
fn a_scheme_out_of_the_set_says_the_chain_id_and_the_length_after() {
    let after = format!("wss://node.example/{SECRET}");
    refused(&anchor(&format!("1={after}")), &format!("E_ARGS #3 (1= + {} bytes)", after.len()), SCHEME);
}

#[test]
fn a_port_past_16_bits_says_the_chain_id_and_the_length_after() {
    let after = format!("https://node.example:65536/{SECRET}");
    refused(&anchor(&format!("1={after}")), &format!("E_ARGS #3 (1= + {} bytes)", after.len()), PORT);
}

#[test]
fn an_empty_port_says_the_chain_id_and_the_length_after() {
    let after = format!("https://node.example:/{SECRET}");
    refused(&anchor(&format!("1={after}")), &format!("E_ARGS #3 (1= + {} bytes)", after.len()), PORT);
}

#[test]
fn a_user_and_password_in_the_address_are_never_echoed() {
    let after = format!("https://user:{SECRET}@node.example/");
    refused(&anchor(&format!("1={after}")), &format!("E_ARGS #3 (1= + {} bytes)", after.len()), ADDRESS);
}

#[test]
fn a_rest_rows_chain_and_base_url_are_read_before_any_node_is_asked() {
    for (after, said) in [
        ("https://api.example/v2/api#rest=5".to_string(), REST_CHAIN),
        ("https://api.example/v2/api#rest=one".to_string(), NOT_INT),
        ("http://api.example/v2/api#rest=1".to_string(), REST_BASE),
        (format!("https://api.example/v2/api?apikey={SECRET}#rest=1"), REST_BASE),
        (format!("https://api.example:65536/{SECRET}#rest=1"), PORT),
    ] {
        refused(&anchor(&format!("1={after}")), &format!("E_ARGS #3 (1= + {} bytes)", after.len()), said);
    }
}

#[test]
fn white_space_inside_the_address_is_named_by_place_and_length() {
    let v = format!("1=https://node.example/a {SECRET}");
    refused(&anchor(&v), &format!("E_ARGS #3 ({} bytes)", v.len()), SHAPE);
}

#[test]
fn a_line_end_inside_the_address_keeps_the_first_line_whole() {
    let v = format!("1=https://node.example/\n{SECRET}");
    refused(&anchor(&v), &format!("E_ARGS #3 ({} bytes)", v.len()), SHAPE);
}

#[test]
fn a_tab_inside_the_address_is_named_by_place_and_length() {
    let v = format!("1=https://node.example/\t{SECRET}");
    refused(&anchor(&v), &format!("E_ARGS #3 ({} bytes)", v.len()), SHAPE);
}

#[test]
fn white_space_after_the_equals_is_named_by_place_and_length() {
    let v = format!("1= https://node.example/{SECRET}");
    refused(&anchor(&v), &format!("E_ARGS #3 ({} bytes)", v.len()), SHAPE);
}

#[test]
fn white_space_before_the_equals_is_named_by_place_and_length() {
    let v = format!("1 =https://node.example/{SECRET}");
    refused(&anchor(&v), &format!("E_ARGS #3 ({} bytes)", v.len()), SHAPE);
}

#[test]
fn the_chain_id_is_said_as_read() {
    refused(&anchor(" +01=wss://h"), "E_ARGS #3 (1= + 7 bytes)", SCHEME);
    refused(&anchor(" +01=wss://h\t"), "E_ARGS #3 (1= + 8 bytes)", SCHEME);
}

#[test]
fn white_space_at_either_end_reads() {
    let port = std::net::TcpListener::bind("127.0.0.1:0").expect("bind").local_addr().expect("addr").port();
    let r = anchor(&format!(" \t31337=http://127.0.0.1:{port}\n "));
    assert_eq!(r.code, 4, "{}", r.err);
}

#[test]
fn scan_names_the_place_and_asks_no_node_before_judging_every_value() {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let good = format!("31337=http://{}", l.local_addr().expect("addr"));
    let asked = Arc::new(AtomicUsize::new(0));
    let a = asked.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            a.fetch_add(1, Ordering::SeqCst);
            drop(s);
        }
    });
    let after = format!("wss://node.example/{SECRET}");
    let r = run(&["scan", "--endpoint", &good, "--endpoint", &format!("31337={after}"), "--basis", "/nowhere"]);
    refused(&r, &format!("E_ARGS #5 (31337= + {} bytes)", after.len()), SCHEME);
    assert_eq!(asked.load(Ordering::SeqCst), 0, "no node asked");
}

#[test]
fn two_good_values_for_anchor_name_the_flag() {
    let hash = format!("0x{}", "11".repeat(32));
    let r = run(&["anchor", "--endpoint", "1=http://127.0.0.1:1", "--endpoint", "2=http://127.0.0.1:2", "--key", KEY, "--form", "registry", "--registry", "0x5fbdb2315678afecb367f032d93f642f64180aa3", "--hash", &hash]);
    refused(&r, "E_ARGS --endpoint", "anchor sends to one chain: every --endpoint names the same chain");
}

#[test]
fn a_value_that_reads_is_not_misuse() {
    let port = std::net::TcpListener::bind("127.0.0.1:0").expect("bind").local_addr().expect("addr").port();
    let r = anchor(&format!("31337=http://127.0.0.1:{port}"));
    assert_eq!(r.code, 4, "{}", r.err);
}

#[test]
fn an_address_that_is_another_equals_is_refused_as_the_app_refuses_it() {
    let after = format!("=https://node.example/{SECRET}");
    refused(&anchor(&format!("1={after}")), &format!("E_ARGS #3 (1= + {} bytes)", after.len()), SCHEME);
}

#[test]
fn a_key_in_a_node_address_is_never_said() {
    let dead = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = dead.local_addr().expect("addr").port();
    drop(dead);
    for value in [format!("31337=http://127.0.0.1:{port}/v3/{SECRET}"), format!("31337=http://127.0.0.1:{port}/rpc?apikey={SECRET}")] {
        let r = anchor(&value);
        let out = String::from_utf8_lossy(&r.out).to_string();
        assert_eq!(r.code, 4, "{value}: {out} {}", r.err);
        assert!(out.contains("E_UNREACHABLE"), "{out}");
        assert!(out.contains(&format!("127.0.0.1:{port}")), "the node is named: {out}");
        assert!(!out.contains(SECRET) && !r.err.contains(SECRET), "the key is said: {out} {}", r.err);
    }
    let value = format!("31337=http://door:{SECRET}@127.0.0.1:{port}/rpc");
    let r = anchor(&value);
    let out = String::from_utf8_lossy(&r.out).to_string();
    assert!(r.code != 0, "{value}: {out} {}", r.err);
    assert!(!out.contains(SECRET) && !r.err.contains(SECRET), "the key is said: {out} {}", r.err);
}
