//! One node written twice is one source (`endpoints::distinct_places`, `thin_chains`, `agree`): addresses are
//! compared by the place they name (`zikaron_net::place_key`), not by their text. Each form below is one line
//! of the table: what makes no second place (the same text, case in the scheme and host, the default port
//! written, the spelling of an IPv6 literal, a missing path against `/`), and what does (another scheme,
//! port, path, a trailing slash, another host name for the same machine, an IPv6 literal against IPv4).

use zikaron::json::Value;
use zikaron_anchor::endpoints::{agree, distinct_places, thin_chains};

#[test]
fn addresses_are_one_place_by_what_they_name() {
    let same = [
        ("the same text", "http://127.0.0.1:8545", "http://127.0.0.1:8545"),
        ("case in the scheme and host", "HTTPS://Node.Example/rpc", "https://node.example/rpc"),
        ("the default port written", "https://node.example:443/rpc", "https://node.example/rpc"),
        ("the http default port written", "http://node.example:80/", "http://node.example/"),
        ("no path and the root path", "http://node.example", "http://node.example/"),
        ("an IPv6 literal spelled out", "http://[0:0:0:0:0:0:0:1]:8545/", "http://[::1]:8545/"),
        ("an IPv6 literal in capitals", "http://[FE80::1]:8545/", "http://[fe80::1]:8545/"),
        ("the same unreadable text", "not an address", "not an address"),
        ("an unreadable text with blanks around it", "  not an address ", "not an address"),
    ];
    for (form, a, b) in same {
        assert_eq!(distinct_places([a, b]), 1, "{form}: one place");
    }
    let apart = [
        ("another scheme", "http://node.example/", "https://node.example/"),
        ("another port", "http://127.0.0.1:8545", "http://127.0.0.1:8546"),
        ("another path", "https://node.example/a", "https://node.example/b"),
        ("a trailing slash", "https://node.example/rpc", "https://node.example/rpc/"),
        ("a path in another case", "https://node.example/Rpc", "https://node.example/rpc"),
        ("another name for the same machine", "http://localhost:8545", "http://127.0.0.1:8545"),
        ("IPv6 against IPv4", "http://[::1]:8545", "http://127.0.0.1:8545"),
        ("two unreadable texts", "not an address", "another text"),
    ];
    for (form, a, b) in apart {
        assert_eq!(distinct_places([a, b]), 2, "{form}: two places");
    }
    assert_eq!(distinct_places(Vec::<&str>::new()), 0, "none");
}

#[test]
fn a_node_written_twice_is_a_single_source() {
    let places = vec![
        (1u64, "https://node.example/rpc".to_string()),
        (1, "HTTPS://NODE.EXAMPLE:443/rpc".to_string()),
        (10, "https://a.example/".to_string()),
        (10, "https://b.example/".to_string()),
    ];
    assert_eq!(thin_chains(&places, &[1, 10, 137]), vec![1, 137], "chain 1 has one place twice; 137 has none");
    let v = Value::Str("0x1".into());
    let twice = agree(vec![("http://127.0.0.1:8545".into(), v.clone()), ("http://127.0.0.1:8545/".into(), v.clone())]).ok().expect("agrees");
    assert!(twice.single_source, "the same node asked twice is one source");
    let two = agree(vec![("http://127.0.0.1:8545".into(), v.clone()), ("http://127.0.0.1:8546".into(), v)]).ok().expect("agrees");
    assert!(!two.single_source, "two nodes are two sources");
}

/// A node is named by scheme, host and port only, but kept apart by its place: two nodes on one host whose
/// paths differ (a key per network in the path) share a name and are two places (what a scan learns of one is
/// kept by place: see the `scan` test `two_nodes_said_alike_keep_their_ranges_apart`).
#[test]
fn two_nodes_on_one_host_share_a_name_and_are_two_places() {
    use zikaron_anchor::rpc::{Endpoint, Http};
    let eth = Http::new("https://rpc.example/eth/KEY1").expect("an address");
    let polygon = Http::new("https://rpc.example/polygon/KEY2").expect("an address");
    assert_eq!(eth.name(), polygon.name(), "said alike: scheme, host and port");
    assert!(!eth.name().contains("KEY"), "the key is not said");
    assert_ne!(eth.place(), polygon.place(), "kept apart");
}
