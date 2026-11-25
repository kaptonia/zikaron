use zikaron_anchor::rpc::{endpoint_spec, NotAnEndpoint};

#[test]
fn an_endpoint_spelling_is_read_one_way() {
    let ok = |s: &str, chain: u64, address: &str| assert_eq!(endpoint_spec(s), Ok((chain, address.to_string())), "{s:?}");
    ok("1=https://a.example", 1, "https://a.example");
    ok(" 1=https://a.example", 1, "https://a.example");
    ok("1=https://a.example ", 1, "https://a.example");
    ok("\t1=https://a.example \n", 1, "https://a.example");
    ok(" \r\n1=https://a.example\r\n\t ", 1, "https://a.example");
    ok("01=https://a.example", 1, "https://a.example");
    ok("+1=https://a.example", 1, "https://a.example");
    ok("0=https://a.example", 0, "https://a.example");
    ok("18446744073709551615=https://a.example", u64::MAX, "https://a.example");
    ok("1=https://a.example/?k=v", 1, "https://a.example/?k=v");
    ok("1==https://a.example", 1, "=https://a.example");
    for (s, why) in [
        ("1", NotAnEndpoint::NoEquals),
        ("", NotAnEndpoint::NoEquals),
        ("https://a.example", NotAnEndpoint::NoEquals),
        ("=https://a.example", NotAnEndpoint::ChainNotInt),
        ("-1=https://a.example", NotAnEndpoint::ChainNotInt),
        ("x=https://a.example", NotAnEndpoint::ChainNotInt),
        ("1.0=https://a.example", NotAnEndpoint::ChainNotInt),
        ("18446744073709551616=https://a.example", NotAnEndpoint::ChainNotInt),
        ("1=", NotAnEndpoint::NoAddress),
        ("1=   ", NotAnEndpoint::NoAddress),
        ("1 =https://a.example", NotAnEndpoint::InnerSpace),
        ("1= https://a.example", NotAnEndpoint::InnerSpace),
        ("1 = https://a.example", NotAnEndpoint::InnerSpace),
        ("1=https://a.example/ x", NotAnEndpoint::InnerSpace),
        ("1=https://a .example", NotAnEndpoint::InnerSpace),
        ("1=\thttps://a.example", NotAnEndpoint::InnerSpace),
        ("1=https://a.example/\tx", NotAnEndpoint::InnerSpace),
        ("1=https://a.example/\nx", NotAnEndpoint::InnerSpace),
        ("1\n=https://a.example", NotAnEndpoint::InnerSpace),
        ("1=https://a.example/\u{a0}x", NotAnEndpoint::InnerSpace),
        (" 1= https://a.example ", NotAnEndpoint::InnerSpace),
        ("1 2=https://a.example", NotAnEndpoint::ChainNotInt),
        ("1= ", NotAnEndpoint::NoAddress),
    ] {
        assert_eq!(endpoint_spec(s), Err(why), "{s:?}");
    }
}

#[test]
fn white_space_at_either_end_is_not_part_of_the_value() {
    let want = Ok((31337, "http://127.0.0.1:8545".to_string()));
    for s in ["31337=http://127.0.0.1:8545", "  31337=http://127.0.0.1:8545", "31337=http://127.0.0.1:8545\t\n", "\u{3000}31337=http://127.0.0.1:8545\u{a0}"] {
        assert_eq!(endpoint_spec(s), want, "{s:?}");
    }
}

#[test]
fn an_item_is_its_spelling_and_its_address() {
    use zikaron_anchor::rpc::{read_address, NotAnAddress};
    let address = |s: &str| endpoint_spec(s).map(|(_, a)| read_address(&a).err());
    assert_eq!(endpoint_spec("1==https://a"), Ok((1, "=https://a".to_string())));
    for (s, why) in [
        ("1==https://a", NotAnAddress::Scheme),
        ("1=wss://a.example", NotAnAddress::Scheme),
        ("1=a.example", NotAnAddress::Scheme),
        ("1=https://a.example:65536", NotAnAddress::Port),
        ("1=https://a.example:", NotAnAddress::Port),
        ("1=https://[zz]:1", NotAnAddress::Host),
        ("1=https://", NotAnAddress::Host),
        ("1=https://u:p@a.example", NotAnAddress::Shape),
    ] {
        assert_eq!(address(s), Ok(Some(why)), "{s:?}");
    }
    for s in ["1=https://a.example", "1=HTTP://a.example:8545/x", "1=https://[::1]:8545"] {
        assert_eq!(address(s), Ok(None), "{s:?}");
    }
}
