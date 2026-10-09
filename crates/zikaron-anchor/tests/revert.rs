//! What a contract said when it reverted (`said::reverted`, `said::reverted_of`): the error's `data` read by
//! Solidity's error encoding, the member that carries the refusal (`Refusal::Reverted`) unchanged. Each form
//! below is one line of the table: the contract's sentence (plain, empty, with control characters, past the
//! cap, not UTF-8, at another offset), each way its body fails to decode, a compiler panic (named, unnamed,
//! too wide, of the wrong length), a declared error, no data, too few bytes; and where `data` sits in the
//! node's error (absent, not text, nested, not hex).

use zikaron_anchor::said::{refusal_of, reverted, reverted_of, Refusal, Reverted, REASON_CAP};

fn word(n: u64) -> Vec<u8> {
    let mut w = vec![0u8; 24];
    w.extend_from_slice(&n.to_be_bytes());
    w
}

/// `Error(string)` encoded with the given offset, length word and text bytes (padded to a word).
fn error_string(offset: u64, len: u64, text: &[u8]) -> Vec<u8> {
    let mut b = vec![0x08, 0xc3, 0x79, 0xa0];
    b.extend(word(offset));
    if offset > 32 {
        b.extend(vec![0u8; (offset - 32) as usize]);
    }
    b.extend(word(len));
    b.extend_from_slice(text);
    b.extend(vec![0u8; (32 - text.len() % 32) % 32]);
    b
}

fn panic_of(code: &[u8]) -> Vec<u8> {
    let mut b = vec![0x4e, 0x48, 0x7b, 0x71];
    b.extend_from_slice(code);
    b
}

#[test]
fn the_revert_data_is_read_by_its_encoding() {
    let long = "x".repeat(REASON_CAP + 44);
    let mut wide = vec![0u8; 32];
    wide[0] = 1;
    let cases: Vec<(&str, Vec<u8>, Reverted)> = vec![
        ("a sentence", error_string(32, 4, b"nope"), Reverted::Text("nope".into())),
        ("an empty sentence", error_string(32, 0, b""), Reverted::Text(String::new())),
        ("control characters escaped", error_string(32, 5, b"a\nb\x07c"), Reverted::Text("a\\nb\\u{7}c".into())),
        ("past the cap", error_string(32, long.len() as u64, long.as_bytes()), Reverted::Text(format!("{}\u{2026} (44 more characters)", "x".repeat(REASON_CAP)))),
        ("not UTF-8", error_string(32, 2, &[0xff, 0x41]), Reverted::Text("\u{fffd}A".into())),
        ("at another offset", error_string(64, 2, b"ok"), Reverted::Text("ok".into())),
        ("an offset past 64 bits", { let mut b = error_string(32, 2, b"ok"); b[4] = 1; b }, Reverted::Unreadable(String::new())),
        ("a length past the end", error_string(32, 99, b"ok"), Reverted::Unreadable(String::new())),
        ("a body cut short", vec![0x08, 0xc3, 0x79, 0xa0, 0, 0], Reverted::Unreadable(String::new())),
        ("a named panic", panic_of(&word(0x11)), Reverted::Panic(0x11)),
        ("an unnamed panic", panic_of(&word(0x99)), Reverted::Panic(0x99)),
        ("a panic code past 64 bits", panic_of(&wide), Reverted::Unreadable(String::new())),
        ("a panic one byte short", panic_of(&word(1)[..31]), Reverted::Unreadable(String::new())),
        ("a panic one byte long", panic_of(&[word(1), vec![0]].concat()), Reverted::Unreadable(String::new())),
        ("a declared error with arguments", [vec![0xde, 0xad, 0xbe, 0xef], word(7), word(8)].concat(), Reverted::Custom([0xde, 0xad, 0xbe, 0xef], 64)),
        ("a declared error without arguments", vec![0xde, 0xad, 0xbe, 0xef], Reverted::Custom([0xde, 0xad, 0xbe, 0xef], 0)),
        ("no data", Vec::new(), Reverted::Silent),
        ("fewer than four bytes", vec![0x08, 0xc3], Reverted::Unreadable(String::new())),
    ];
    for (form, bytes, want) in cases {
        let got = reverted_of(&bytes);
        match (&got, &want) {
            // An unreadable body is said with its hex, as given.
            (Reverted::Unreadable(h), Reverted::Unreadable(_)) => assert_eq!(h, &zikaron::hexfmt::encode(&bytes), "{form}"),
            _ => assert_eq!(got, want, "{form}"),
        }
    }
    assert_eq!(reverted_of(&panic_of(&word(0x11))).evidence(), "panic 0x11: arithmetic overflow or underflow");
    assert_eq!(reverted_of(&panic_of(&word(0x99))).evidence(), "panic 0x99");
    assert_eq!(reverted_of(&error_string(32, 4, b"nope")).evidence(), "reason: nope");
    assert_eq!(reverted_of(&[]).evidence(), "no reason given");
}

#[test]
fn the_data_is_found_where_nodes_put_it_and_the_member_stays_the_same() {
    let hex = zikaron::hexfmt::encode(&error_string(32, 4, b"nope"));
    let flat = format!("{{\"code\":3,\"message\":\"execution reverted: nope\",\"data\":\"{hex}\"}}");
    let nested = format!("{{\"code\":3,\"message\":\"execution reverted\",\"data\":{{\"data\":\"{hex}\"}}}}");
    for (form, err, want) in [
        ("flat", flat.as_str(), Some(Reverted::Text("nope".into()))),
        ("nested", nested.as_str(), Some(Reverted::Text("nope".into()))),
        ("no data", "{\"code\":3,\"message\":\"execution reverted\"}", None),
        ("data not text", "{\"code\":3,\"message\":\"execution reverted\",\"data\":7}", None),
        ("data not hex", "{\"code\":3,\"message\":\"execution reverted\",\"data\":\"0xzz\"}", None),
        ("data with a capital prefix", "{\"code\":3,\"message\":\"execution reverted\",\"data\":\"0X08\"}", None),
        ("empty data", "{\"code\":3,\"message\":\"execution reverted\",\"data\":\"0x\"}", Some(Reverted::Silent)),
        ("not JSON", "execution reverted", None),
    ] {
        assert_eq!(reverted(err), want, "{form}");
        assert_eq!(refusal_of(err), Refusal::Reverted, "{form}: the member is the same");
    }
}
