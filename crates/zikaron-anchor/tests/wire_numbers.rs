//! A number in a node's answer is read by JSON's own grammar (RFC 8259): an integer part of `0` or one that
//! starts with 1 to 9. A leading zero (`-032005`, `03`) is not a number, so the answer does not read (never a
//! code read as another); the fraction and the exponent keep their own digits.

#[test]
fn a_number_with_a_leading_zero_is_not_a_number() {
    use zikaron_anchor::wire::parse;
    for ok in ["0", "-0", "7", "-32005", "0.5", "-0.25", "1e05", "1E+3", "10", "100"] {
        assert!(parse(ok.as_bytes()).is_some(), "{ok}");
    }
    for bad in ["-032005", "03", "00", "-00", "012.5", "+3", "-", ".5", "1.", "1e"] {
        assert!(parse(bad.as_bytes()).is_none(), "{bad}");
    }
    assert!(parse(br#"{"error":{"code":-032005,"message":"x"},"id":1,"jsonrpc":"2.0"}"#).is_none(), "the whole answer does not read");
    assert!(parse(br#"{"error":{"code":-32005,"message":"x"},"id":1,"jsonrpc":"2.0"}"#).is_some());
}
