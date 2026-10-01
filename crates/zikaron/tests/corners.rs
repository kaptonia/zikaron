//! Unit tests of the corners of law §3 to §9 that the reference implementations do not reach. Every sample is
//! built and signed inside the tests; nothing is copied from `base/`.

use zikaron::audit;
use zikaron::cryptox;
use zikaron::entry;
use zikaron::hexfmt;
use zikaron::json::{self, Value};

const KEY_A: [u8; 32] = [0x11; 32];
const KEY_B: [u8; 32] = [0x22; 32];

fn addr(key: &[u8; 32]) -> String {
    hexfmt::encode(&cryptox::address_of_privkey(key).unwrap())
}

fn o(pairs: Vec<(&str, Value)>) -> Value {
    Value::Obj(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn st(x: &str) -> Value {
    Value::Str(x.to_string())
}

/// Build a signed entry: take the §5.1 preimage of the six-member envelope, sign, add `sig`, return the
/// canonical bytes.
fn signed(key: &[u8; 32], entry_type: &str, seq: u64, prev: Option<&str>, body: Value) -> Vec<u8> {
    let author = addr(key);
    let six = o(vec![
        ("spec", st("zikaron/1")),
        ("entryType", st(entry_type)),
        ("author", st(&author)),
        ("seq", Value::Int(seq)),
        ("prev", match prev {
            Some(p) => st(p),
            None => Value::Null,
        }),
        ("body", body),
    ]);
    let b6 = json::canon_bytes(&six);
    let (_, digest) = entry::presig_and_digest(&b6, "zikaron/1");
    let (r, s, v) = cryptox::sign_digest(key, &digest).unwrap();
    let mut sig = Vec::new();
    sig.extend_from_slice(&r);
    sig.extend_from_slice(&s);
    sig.push(v);
    let mut ms = match six {
        Value::Obj(m) => m,
        _ => unreachable!(),
    };
    ms.push(("sig".to_string(), st(&hexfmt::encode(&sig))));
    json::canon_bytes(&Value::Obj(ms))
}

fn genesis(key: &[u8; 32]) -> Vec<u8> {
    signed(key, "genesis", 0, None, o(vec![("statement_md", st("a ledger"))]))
}

fn id_of(b: &[u8]) -> String {
    hexfmt::encode(&entry::entry_id(b))
}

// §3 canonical form.

#[test]
fn canon_sorts_members_bytewise_and_emits_no_whitespace() {
    let v = json::parse_tests_1_5(b"{ \"b\" : 1 , \"a\" : [ 1 , 2 ] }").unwrap();
    assert_eq!(json::canon_bytes(&v), b"{\"a\":[1,2],\"b\":1}".to_vec());
}

#[test]
fn canon_escapes_only_the_seven_and_leaves_del_raw() {
    let v = json::parse_tests_1_5("{\"k_md\":\"\\u0000\\u001f\\t\\u007f\"}".as_bytes()).unwrap();
    let out = json::canon_bytes(&v);
    assert_eq!(out, "{\"k_md\":\"\\u0000\\u001f\\t\u{7f}\"}".as_bytes().to_vec());
}

#[test]
fn accept_is_roundtrip_identity() {
    assert!(json::accept(b"{\"a\":1}").is_ok());
    assert_eq!(json::accept(b"{\"a\":1}\n").unwrap_err().as_str(), "E_NOT_CANONICAL");
    assert_eq!(json::accept(b" {\"a\":1}").unwrap_err().as_str(), "E_NOT_CANONICAL");
    assert_eq!(json::accept(b"{\"b\":1,\"a\":2}").unwrap_err().as_str(), "E_NOT_CANONICAL");
}

#[test]
fn bom_is_not_whitespace() {
    assert_eq!(json::accept(b"\xef\xbb\xbf{}").unwrap_err().as_str(), "E_JSON");
}

#[test]
fn numeric_shape_faults_are_e_number_and_others_e_json() {
    assert_eq!(json::parse(b"01").unwrap_err().as_str(), "E_NUMBER");
    assert_eq!(json::parse(b"-Infinity").unwrap_err().as_str(), "E_NUMBER");
    assert_eq!(json::parse(b"1.5").unwrap_err().as_str(), "E_NUMBER");
    assert_eq!(json::parse(b"9007199254740992").unwrap_err().as_str(), "E_NUMBER");
    assert!(json::parse(b"9007199254740991").is_ok());
    // `0x10`: the number bytes are `0`, an integer; `x` is the fault, so E_JSON.
    assert_eq!(json::parse(b"0x10").unwrap_err().as_str(), "E_JSON");
    assert_eq!(json::parse(b"NaN").unwrap_err().as_str(), "E_JSON");
}

#[test]
fn depth_128_stands_and_129_fails() {
    let mut ok = vec![b'['; 128];
    ok.extend(vec![b']'; 128]);
    assert!(json::parse(&ok).is_ok());
    let mut red = vec![b'['; 129];
    red.extend(vec![b']'; 129]);
    assert_eq!(json::parse(&red).unwrap_err().as_str(), "E_DEPTH");
}

#[test]
fn utf8_beats_every_later_test() {
    let mut b = vec![b'['; 200];
    b.push(0xff);
    assert_eq!(json::parse(&b).unwrap_err().as_str(), "E_UTF8");
}

#[test]
fn surrogate_escape_is_e_json_even_as_a_pair() {
    assert_eq!(json::parse(b"\"\\ud83d\\ude00\"").unwrap_err().as_str(), "E_JSON");
}

#[test]
fn duplicate_keys_then_key_charset_then_value_charset() {
    assert_eq!(json::parse_tests_1_5(b"{\"a\":1,\"a\":2}").unwrap_err().as_str(), "E_DUP_KEY");
    assert_eq!(json::parse_tests_1_5(b"{\"\":1}").unwrap_err().as_str(), "E_KEY_CHARSET");
    assert_eq!(
        json::parse_tests_1_5("{\"a\":\"\u{e9}\"}".as_bytes()).unwrap_err().as_str(),
        "E_VALUE_CHARSET"
    );
    // Prose subtree: under a key ending in `_md`, string values at any depth pass.
    assert!(json::parse_tests_1_5("{\"a_md\":{\"b\":[\"\u{e9}\"]}}".as_bytes()).is_ok());
}

// §4 to §6: envelope and body.

#[test]
fn a_self_signed_genesis_is_an_entry() {
    let b = genesis(&KEY_A);
    let e = entry::check(&b).expect("genesis accepted");
    assert_eq!(e.author, addr(&KEY_A));
    assert_eq!(e.seq, 0);
    assert!(e.prev.is_none());
}

#[test]
fn decision_order_names_the_first_fault() {
    let b = genesis(&KEY_A);
    let s = String::from_utf8(b.clone()).unwrap();
    // An eighth key: E_ENVELOPE_CLOSED.
    let closed = s.replace("{\"author\"", "{\"aextra\":1,\"author\"");
    assert_eq!(entry::check(closed.as_bytes()).unwrap_err().as_str(), "E_ENVELOPE_CLOSED");
    // A missing key: E_ENVELOPE_MISSING (judged before E_ENVELOPE_CLOSED).
    let missing = json::canon_bytes(&Value::Obj(vec![("spec".to_string(), st("zikaron/1"))]));
    assert_eq!(entry::check(&missing).unwrap_err().as_str(), "E_ENVELOPE_MISSING");
    assert_eq!(entry::check(b"[]").unwrap_err().as_str(), "E_ENVELOPE");
    // spec before entryType.
    let spec = s.replace("\"spec\":\"zikaron/1\"", "\"spec\":\"zikaron/2\"");
    assert_eq!(entry::check(spec.as_bytes()).unwrap_err().as_str(), "E_SPEC");
}

#[test]
fn genesis_placement_and_prev_seq() {
    // seq 0 but not genesis: E_GENESIS_PLACE.
    let b = signed(&KEY_A, "history", 0, None, o(vec![]));
    assert_eq!(entry::check(&b).unwrap_err().as_str(), "E_GENESIS_PLACE");
    // seq 1 with a null prev: E_PREV_SEQ, judged before the genesis position.
    let b = signed(&KEY_A, "genesis", 1, None, o(vec![("statement_md", st("x"))]));
    assert_eq!(entry::check(&b).unwrap_err().as_str(), "E_PREV_SEQ");
}

#[test]
fn body_tables_are_one_token() {
    let g = genesis(&KEY_A);
    let head = id_of(&g);
    // history without mode.
    let b = signed(
        &KEY_A,
        "history",
        1,
        Some(&head),
        o(vec![("content", st(&hexfmt::encode(&[0u8; 32])))]),
    );
    assert_eq!(entry::check(&b).unwrap_err().as_str(), "E_BODY_FIELD");
    // A grant window running backwards.
    let b = signed(
        &KEY_A,
        "grant",
        1,
        Some(&head),
        o(vec![
            ("grantee", st(&addr(&KEY_B))),
            ("work", st(&hexfmt::encode(&[1u8; 32]))),
            ("terms", st(&hexfmt::encode(&[2u8; 32]))),
            ("window", o(vec![("from", Value::Int(9)), ("to", Value::Int(8))])),
        ]),
    );
    assert_eq!(entry::check(&b).unwrap_err().as_str(), "E_BODY_FIELD");
    // An optional field written as null is present in its refused shape.
    let b = signed(
        &KEY_A,
        "annotation",
        1,
        Some(&head),
        o(vec![("subject", Value::Null), ("note_md", st("n"))]),
    );
    assert_eq!(entry::check(&b).unwrap_err().as_str(), "E_BODY_FIELD");
    // Unlisted type: an object body suffices (§6.9).
    let b = signed(&KEY_A, "weather", 1, Some(&head), o(vec![("anything", Value::Int(1))]));
    assert!(entry::check(&b).is_ok());
}

#[test]
fn signature_boundaries() {
    let g = genesis(&KEY_A);
    let s = String::from_utf8(g.clone()).unwrap();
    let sig_start = s.find("\"sig\":\"0x").unwrap() + 9;
    let mut bytes = s.clone().into_bytes();
    // v changed to 1: E_SIG_V.
    let v_at = sig_start + 128;
    bytes[v_at] = b'0';
    bytes[v_at + 1] = b'1';
    assert_eq!(entry::check(&bytes).unwrap_err().as_str(), "E_SIG_V");
    // r all zero: E_SIG_RANGE.
    let mut bytes = s.clone().into_bytes();
    for i in 0..64 {
        bytes[sig_start + i] = b'0';
    }
    assert_eq!(entry::check(&bytes).unwrap_err().as_str(), "E_SIG_RANGE");
    // Someone else's signature: E_SIG_SIGNER or E_SIG_RECOVER; not an entry either way.
    let other = genesis(&KEY_B);
    let os = String::from_utf8(other).unwrap();
    let osig = &os[os.find("\"sig\":\"0x").unwrap() + 7..];
    let osig = &osig[..132];
    let swapped = s.replace(&s[sig_start - 9 + 7..sig_start - 9 + 7 + 132], osig);
    assert_eq!(entry::check(swapped.as_bytes()).unwrap_err().as_str(), "E_SIG_SIGNER");
}

#[test]
fn sig_is_low_s_and_recovers_to_the_signer() {
    let digest = [7u8; 32];
    let (r, s2, v) = cryptox::sign_digest(&KEY_A, &digest).unwrap();
    assert!(cryptox::in_range(&r) && cryptox::in_range(&s2));
    assert!(cryptox::is_low_s(&s2));
    assert!(v == 27 || v == 28);
    let a = cryptox::recover_address(&digest, &r, &s2, v - 27).unwrap();
    assert_eq!(hexfmt::encode(&a), addr(&KEY_A));
}

#[test]
fn entry_message_is_76_bytes_and_adoption_is_85() {
    let presig = [0u8; 32];
    assert_eq!(entry::message("zikaron/1", &presig).len(), 76);
    assert_eq!(entry::message("zikaron/1-adoption", &presig).len(), 85);
}

// §8 audit.

fn basis_empty() -> Value {
    o(vec![
        ("chains", Value::Arr(vec![])),
        ("bareTx", Value::Arr(vec![])),
        ("adoptionChains", Value::Arr(vec![])),
    ])
}

fn audit_input(root: &str, pile: Vec<Vec<u8>>, basis: Value) -> Value {
    o(vec![
        ("root", st(root)),
        (
            "pile",
            Value::Arr(pile.iter().map(|b| st(&hexfmt::encode(b))).collect()),
        ),
        ("anchors", Value::Arr(vec![])),
        ("unavailable", Value::Arr(vec![])),
        ("evidence", Value::Arr(vec![])),
        ("basis", basis),
    ])
}

#[test]
fn a_clean_two_entry_ledger_is_complete_and_unanchored() {
    let g = genesis(&KEY_A);
    let head = id_of(&g);
    let n = signed(&KEY_A, "annotation", 1, Some(&head), o(vec![("note_md", st("hello"))]));
    let rep = audit::audit(&audit_input(&addr(&KEY_A), vec![g, n], basis_empty()));
    assert_eq!(rep.member("label").unwrap().as_str().unwrap(), "COMPLETE");
    assert_eq!(rep.member("entries").unwrap().as_int().unwrap(), 2);
    assert_eq!(rep.member("unanchored").unwrap().as_arr().unwrap().len(), 2);
    assert!(rep.member("findings").unwrap().as_arr().unwrap().is_empty());
}

#[test]
fn a_fork_at_one_seq_is_broken_chain() {
    let g = genesis(&KEY_A);
    let head = id_of(&g);
    let a = signed(&KEY_A, "annotation", 1, Some(&head), o(vec![("note_md", st("a"))]));
    let b = signed(&KEY_A, "annotation", 1, Some(&head), o(vec![("note_md", st("b"))]));
    let rep = audit::audit(&audit_input(&addr(&KEY_A), vec![g, a, b], basis_empty()));
    assert_eq!(rep.member("label").unwrap().as_str().unwrap(), "BROKEN_CHAIN");
    let f = rep.member("findings").unwrap().as_arr().unwrap();
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].member("name").unwrap().as_str().unwrap(), "EQUIVOCATION");
}

#[test]
fn entries_outside_the_lineage_are_excluded_and_convict_nobody() {
    let g = genesis(&KEY_A);
    let foreign = genesis(&KEY_B);
    let rep = audit::audit(&audit_input(&addr(&KEY_A), vec![g, foreign], basis_empty()));
    assert_eq!(rep.member("label").unwrap().as_str().unwrap(), "COMPLETE");
    assert_eq!(rep.member("entries").unwrap().as_int().unwrap(), 1);
    assert_eq!(rep.member("excluded").unwrap().as_arr().unwrap().len(), 1);
}

#[test]
fn a_succession_moves_authority_and_the_successor_is_input() {
    let g = genesis(&KEY_A);
    let head = id_of(&g);
    let succ = signed(
        &KEY_A,
        "succession",
        1,
        Some(&head),
        o(vec![
            ("to", st(&addr(&KEY_B))),
            ("kind", st("handover")),
            ("effective", Value::Int(0)),
            ("statement_md", st("handing over")),
        ]),
    );
    let head2 = id_of(&succ);
    let after = signed(&KEY_B, "annotation", 2, Some(&head2), o(vec![("note_md", st("mine now"))]));
    let rep = audit::audit(&audit_input(&addr(&KEY_A), vec![g, succ, after], basis_empty()));
    assert_eq!(rep.member("label").unwrap().as_str().unwrap(), "COMPLETE");
    assert_eq!(rep.member("entries").unwrap().as_int().unwrap(), 3);
}

#[test]
fn a_missing_seq_zero_records_a_gap_and_labels_gaps() {
    let g = genesis(&KEY_A);
    let head = id_of(&g);
    let n = signed(&KEY_A, "annotation", 1, Some(&head), o(vec![("note_md", st("x"))]));
    let rep = audit::audit(&audit_input(&addr(&KEY_A), vec![n], basis_empty()));
    assert_eq!(rep.member("label").unwrap().as_str().unwrap(), "GAPS");
    let f = rep.member("findings").unwrap().as_arr().unwrap();
    assert_eq!(f[0].member("name").unwrap().as_str().unwrap(), "SEQ_GAP");
    assert_eq!(f[0].member("hard").unwrap(), &Value::Bool(false));
}

#[test]
fn a_basis_with_an_extra_member_is_no_audit_input() {
    let g = genesis(&KEY_A);
    let mut basis = match basis_empty() {
        Value::Obj(m) => m,
        _ => unreachable!(),
    };
    basis.push(("extra".to_string(), Value::Int(1)));
    let rep = audit::audit(&audit_input(&addr(&KEY_A), vec![g], Value::Obj(basis)));
    assert_eq!(rep.member("reason").unwrap().as_str().unwrap(), "NO_LABEL");
    assert!(rep.member("label").is_none());
}

#[test]
fn an_anchor_verdict_outside_the_closed_set_is_no_audit_input() {
    let g = genesis(&KEY_A);
    let mut inp = match audit_input(&addr(&KEY_A), vec![g], basis_empty()) {
        Value::Obj(m) => m,
        _ => unreachable!(),
    };
    for m in inp.iter_mut() {
        if m.0 == "anchors" {
            m.1 = Value::Arr(vec![o(vec![
                ("chainId", Value::Int(1)),
                ("blockNumber", Value::Int(1)),
                ("blockTimestamp", Value::Int(1)),
                ("tx", st(&hexfmt::encode(&[3u8; 32]))),
                ("sender", st(&addr(&KEY_A))),
                ("hash", st(&hexfmt::encode(&[4u8; 32]))),
                ("verdict", st("maybe")),
            ])]);
        }
    }
    let rep = audit::audit(&Value::Obj(inp));
    assert_eq!(rep.member("reason").unwrap().as_str().unwrap(), "NO_LABEL");
}

#[test]
fn an_anchor_from_outside_the_lineage_is_discarded() {
    let g = genesis(&KEY_A);
    // Reach (law §9.4): the record must be within the basis, so bareTx names this transaction (a bare anchor
    // ignores senders).
    let basis = o(vec![
        ("chains", Value::Arr(vec![])),
        (
            "bareTx",
            Value::Arr(vec![o(vec![
                ("chainId", Value::Int(1)),
                ("tx", st(&hexfmt::encode(&[3u8; 32]))),
            ])]),
        ),
        ("adoptionChains", Value::Arr(vec![])),
    ]);
    let mut inp = match audit_input(&addr(&KEY_A), vec![g], basis) {
        Value::Obj(m) => m,
        _ => unreachable!(),
    };
    for m in inp.iter_mut() {
        if m.0 == "anchors" {
            m.1 = Value::Arr(vec![o(vec![
                ("chainId", Value::Int(1)),
                ("blockNumber", Value::Int(1)),
                ("blockTimestamp", Value::Int(1)),
                ("tx", st(&hexfmt::encode(&[3u8; 32]))),
                ("sender", st(&addr(&KEY_B))),
                ("hash", st(&hexfmt::encode(&[4u8; 32]))),
                ("verdict", st("counted")),
            ])]);
        }
    }
    let rep = audit::audit(&Value::Obj(inp));
    // A sender outside the lineage cannot put a hash into this ledger's reconciliation: MISSING is empty and
    // the label stays COMPLETE; the discarded record is listed with its seven members in DISCARDED (§8.7 item
    // 14).
    assert!(rep.member("missing").unwrap().as_arr().unwrap().is_empty());
    assert_eq!(rep.member("label").unwrap().as_str().unwrap(), "COMPLETE");
    let discarded = rep.member("discarded").unwrap().as_arr().unwrap();
    assert_eq!(discarded.len(), 1);
    assert_eq!(discarded[0].member("sender").unwrap().as_str().unwrap(), addr(&KEY_B));
    assert_eq!(discarded[0].member("verdict").unwrap().as_str().unwrap(), "counted");
}
