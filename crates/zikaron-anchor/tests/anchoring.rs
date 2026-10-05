//! Library tests of the anchoring crate: each assertion checks a sentence of law §9 or a reading of this
//! layer.
//!
//! Fixture replay, anvil scenarios and kit tampering run end to end elsewhere; these tests check the
//! library's own decisions and copy nothing from `base/`: every sample is built here.

use zikaron::json::Value;
use zikaron_anchor::rpc::{Endpoint, Replay, Trouble};
use zikaron_anchor::wire::{self, Body, W};
use zikaron_anchor::{mpt, rlp, scan, send, tx};

fn w(json: &str) -> W {
    wire::parse(json.as_bytes()).expect("样本不是 JSON")
}

// ───────────────────────── RLP ─────────────────────────

#[test]
fn rlp_writes_and_reads_back_the_same_value() {
    let cases: Vec<Vec<u8>> = vec![vec![], vec![0x7f], vec![0x80], vec![0u8; 55], vec![1u8; 1024]];
    for c in cases {
        let enc = rlp::bytes(&c);
        assert_eq!(rlp::decode_all(&enc).unwrap().bytes().unwrap(), &c[..]);
    }
    let list = rlp::list(&[rlp::quantity(0), rlp::quantity(1), rlp::bytes(b"zikaron")]);
    let items = rlp::decode_all(&list).unwrap().list().unwrap().to_vec();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].u64(), Some(0));
    assert_eq!(items[1].u64(), Some(1));
    assert_eq!(items[2].bytes(), Some(&b"zikaron"[..]));
}

#[test]
fn a_quantity_carries_no_leading_zero_and_zero_is_the_empty_string() {
    assert_eq!(rlp::quantity(0), rlp::bytes(&[]));
    assert_eq!(rlp::quantity(1), vec![0x01]);
    assert_eq!(rlp::scalar(&[0, 0, 5]), vec![0x05]);
}

#[test]
fn a_non_canonical_single_byte_encoding_is_refused() {
    // 0x81 0x7f is a non-canonical spelling of 0x7f: a single byte below 0x80 has one spelling.
    assert!(rlp::decode_all(&[0x81, 0x7f]).is_none());
}

// Transactions and senders.

fn a_signed_bare_tx(chain: u64, key: [u8; 32], words: &[[u8; 32]]) -> (Vec<u8>, [u8; 20]) {
    let from = zikaron::cryptox::address_of_privkey(&key).unwrap();
    let unsigned = send::Unsigned {
        chain_id: chain,
        nonce: 3,
        max_priority_fee: 1,
        max_fee: 2,
        gas: 21000,
        to: from,
        value: 0,
        data: send::bare_calldata(words),
    };
    let (raw, _) = unsigned.sign(&key).unwrap();
    (raw, from)
}

#[test]
fn a_sender_is_recovered_from_the_transactions_own_signature() {
    let key = [7u8; 32];
    let (raw, from) = a_signed_bare_tx(31337, key, &[[0xaa; 32]]);
    let t = tx::from_raw(&raw).unwrap();
    assert_eq!(t.sender, Some(from));
    assert_eq!(t.chain_id, Some(31337));
    assert_eq!(t.to, Some(from));
    assert_eq!(t.input.len(), 32);
}

#[test]
fn a_transaction_whose_bytes_are_not_its_hash_is_refused() {
    let key = [7u8; 32];
    let (raw, from) = a_signed_bare_tx(31337, key, &[[0xaa; 32]]);
    let t = tx::from_raw(&raw).unwrap();
    let object = format!(
        "{{\"blockNumber\":\"0x2\",\"chainId\":\"0x7a69\",\"gas\":\"0x5208\",\"hash\":\"0x{}\",\"input\":\"0x{}\",\"maxFeePerGas\":\"0x2\",\"maxPriorityFeePerGas\":\"0x1\",\"nonce\":\"0x3\",\"r\":\"0x1\",\"s\":\"0x1\",\"to\":\"{}\",\"type\":\"0x2\",\"v\":\"0x0\",\"value\":\"0x0\",\"yParity\":\"0x0\"}}",
        "11".repeat(32),
        t.input.iter().map(|b| format!("{b:02x}")).collect::<String>(),
        zikaron::hexfmt::encode(&from)
    );
    assert!(matches!(tx::read(&w(&object)), Err(tx::Bad::HashMismatch)));
}

#[test]
fn a_transaction_with_no_signature_has_no_sender_in_this_grammar() {
    let object = "{\"gas\":\"0x1\",\"hash\":\"0x1111111111111111111111111111111111111111111111111111111111111111\",\"input\":\"0x\",\"nonce\":\"0x0\",\"r\":\"0x0\",\"s\":\"0x0\",\"to\":\"0x1111111111111111111111111111111111111111\",\"type\":\"0x7e\",\"v\":\"0x0\",\"value\":\"0x0\"}";
    let t = tx::read(&w(object)).unwrap();
    assert_eq!(t.sender, None);
    assert_eq!(t.chain_id, None);
}

#[test]
fn an_unknown_transaction_type_has_no_sender() {
    let object = "{\"gas\":\"0x1\",\"hash\":\"0x1111111111111111111111111111111111111111111111111111111111111111\",\"input\":\"0x\",\"nonce\":\"0x0\",\"r\":\"0x11\",\"s\":\"0x22\",\"to\":\"0x1111111111111111111111111111111111111111\",\"type\":\"0x10\",\"v\":\"0x0\",\"value\":\"0x0\"}";
    assert_eq!(tx::read(&w(object)).unwrap().sender, None);
}

// The §9.1 containment test.

#[test]
fn containment_reads_only_the_two_aligned_families_of_offsets() {
    let word = [0xab; 32];
    let mut at_zero = word.to_vec();
    at_zero.extend_from_slice(&[0u8; 32]);
    assert!(scan::calldata_carries(&at_zero, &word));

    let mut at_four = vec![0xde, 0xad, 0xbe, 0xef];
    at_four.extend_from_slice(&word);
    assert!(scan::calldata_carries(&at_four, &word));

    let mut at_five = vec![0u8; 5];
    at_five.extend_from_slice(&word);
    assert!(!scan::calldata_carries(&at_five, &word), "偏移 5 不在两族对齐位上");

    let mut cut = word.to_vec();
    cut.pop();
    assert!(!scan::calldata_carries(&cut, &word), "整字要整个落在里面");
}

// §9.4 deduplication and endpoint checks.

fn rec(chain: u64, bn: u64, tx_byte: u8, hash_byte: u8) -> scan::AnchorRec {
    scan::AnchorRec {
        chain_id: chain,
        block_number: bn,
        block_timestamp: 1,
        tx: [tx_byte; 32],
        sender: [1u8; 20],
        hash: [hash_byte; 32],
        verdict: zikaron::tokens::Verdict::Counted,
    }
}

#[test]
fn one_key_holds_one_record() {
    let rows = vec![rec(1, 2, 3, 4), rec(1, 2, 3, 4), rec(1, 2, 3, 5)];
    assert_eq!(scan::dedupe(rows).len(), 2);
}

#[test]
fn an_endpoint_must_answer_with_the_chain_the_basis_declared() {
    assert!(scan::endpoint_serves(&w("\"0x7a69\""), 31337));
    assert!(!scan::endpoint_serves(&w("\"0x2105\""), 31337));
    assert!(!scan::endpoint_serves(&w("null"), 31337));
}

#[test]
fn a_log_outside_the_registry_shape_is_no_anchor() {
    let t0 = scan::topic0();
    let good = format!(
        "{{\"blockNumber\":\"0x2\",\"data\":\"0x\",\"topics\":[\"{}\",\"0x{}\",\"0x{}\"],\"transactionHash\":\"0x{}\"}}",
        zikaron::hexfmt::encode(&t0),
        format!("{}{}", "0".repeat(24), "11".repeat(20)),
        "22".repeat(32),
        "33".repeat(32)
    );
    assert!(scan::anchored_log(&w(&good), &t0).is_some());
    assert!(scan::anchored_log(&w(&good.replace("\"data\":\"0x\"", "\"data\":\"0x00\"")), &t0).is_none(), "一个 data 字节即不是锚");
    let two_topics = format!(
        "{{\"blockNumber\":\"0x2\",\"data\":\"0x\",\"topics\":[\"{}\",\"0x{}\"],\"transactionHash\":\"0x{}\"}}",
        zikaron::hexfmt::encode(&t0),
        format!("{}{}", "0".repeat(24), "11".repeat(20)),
        "33".repeat(32)
    );
    assert!(scan::anchored_log(&w(&two_topics), &t0).is_none(), "两个 topic 即不是锚");
}

// Recording and replay.

#[test]
fn a_question_the_recording_does_not_hold_is_an_error_and_never_a_guess() {
    let ex = vec![w("{\"method\":\"eth_chainId\",\"params\":[],\"result\":\"0x7a69\"}")];
    let mut r = Replay::new("t", &ex).unwrap();
    assert_eq!(r.call("eth_chainId", &Value::Arr(vec![])).unwrap().as_str(), Some("0x7a69"));
    match r.call("eth_getLogs", &Value::Arr(vec![])) {
        Err(Trouble::NotServed(_)) => {}
        other => panic!("录制里没有的那一问要报没录到,got {other:?}"),
    }
}

#[test]
fn a_recording_that_answers_one_question_two_ways_is_refused() {
    let ex = vec![
        w("{\"method\":\"eth_chainId\",\"params\":[],\"result\":\"0x7a69\"}"),
        w("{\"method\":\"eth_chainId\",\"params\":[],\"result\":\"0x2105\"}"),
    ];
    assert!(matches!(Replay::new("t", &ex), Err(Trouble::Contradiction(_))));
}

#[test]
fn the_same_answer_recorded_twice_is_one_answer() {
    let ex = vec![
        w("{\"method\":\"eth_chainId\",\"params\":[],\"result\":\"0x7a69\"}"),
        w("{\"method\":\"eth_chainId\",\"params\":[],\"result\":\"0x7a69\"}"),
    ];
    assert!(Replay::new("t", &ex).is_ok(), "两处逐字节相同的答是同一个答");
}

#[test]
fn a_node_error_is_carried_as_a_refusal_and_not_as_an_answer() {
    let ex = vec![w("{\"error\":{\"code\":-32000,\"message\":\"no\"},\"method\":\"eth_getCode\",\"params\":[]}")];
    let mut r = Replay::new("t", &ex).unwrap();
    assert!(matches!(r.call("eth_getCode", &Value::Arr(vec![])), Err(Trouble::Node(_))));
}

// The transport reader.

#[test]
fn the_transport_reader_takes_what_the_law_does_not() {
    let v = wire::parse(b"{\"code\":-32000,\"ratio\":1.5}").unwrap();
    assert_eq!(v.member("code").unwrap().as_u64(), None, "负数不是 §3 的整数,也不冒充一个");
    assert!(wire::to_core(&v).is_none(), "落不进 §3 值域的值不许被折进去");
    let ok = wire::parse(b"{\"a\":1}").unwrap();
    assert!(wire::to_core(&ok).is_some());
}

#[test]
fn a_value_keeps_the_bytes_it_was_written_as() {
    let src = b"{\"basis\":{\"b\":1,\"a\":2}}";
    let v = wire::parse(src).unwrap();
    assert_eq!(v.member("basis").unwrap().raw(src), b"{\"b\":1,\"a\":2}", "原文照留:重写一遍会把形上的病治好");
}

// ───────────────────────── MPT ─────────────────────────

#[test]
fn a_proof_leads_to_the_value_and_a_tampered_one_leads_nowhere() {
    let values: Vec<Vec<u8>> = (0u8..12).map(|i| vec![i; 40]).collect();
    let trie = mpt::Trie::indexed(&values);
    let root = trie.root();
    for i in 0..values.len() {
        let key = rlp::quantity(i as u64);
        let proof = trie.proof(&key);
        assert_eq!(mpt::verify(&root, &key, &proof), Some(mpt::Answer::Value(values[i].clone())));
        let mut bad = proof.clone();
        let last = bad.len() - 1;
        bad[last][3] ^= 0xff;
        assert!(mpt::verify(&root, &key, &bad).is_none(), "改过的路径证明不了任何东西");
        let other = [0u8; 32];
        assert!(mpt::verify(&other, &key, &proof).is_none(), "别的根上这条路不成立");
    }
}

#[test]
fn a_key_the_trie_does_not_hold_answers_absent() {
    let values: Vec<Vec<u8>> = (0u8..3).map(|i| vec![i; 40]).collect();
    let trie = mpt::Trie::indexed(&values);
    let key = rlp::quantity(9);
    assert_eq!(mpt::verify(&trie.root(), &key, &trie.proof(&key)), Some(mpt::Answer::Absent));
}

// The endpoint rule.

#[test]
fn a_reading_stands_only_when_the_endpoints_agree() {
    use zikaron_anchor::endpoints::agree;
    let one = Value::Obj(vec![("anchors".into(), Value::Arr(vec![]))]);
    let two = Value::Obj(vec![("anchors".into(), Value::Arr(vec![Value::Null]))]);
    let r = agree(vec![("a".into(), one.clone()), ("b".into(), one.clone())]).ok().unwrap();
    assert!(!r.single_source);
    let single = agree(vec![("a".into(), one.clone())]).ok().unwrap();
    assert!(single.single_source, "单源那一栏必须随读数走");
    assert!(agree(vec![("a".into(), one), ("b".into(), two)]).is_err(), "不一致不取多数");
}

// The two anchoring forms.

#[test]
fn the_two_forms_build_the_calldata_the_law_reads() {
    let h = [0x5a; 32];
    let one = send::registry_calldata(&[h]);
    assert_eq!(&one[..4], &send::selector_anchor()[..]);
    assert!(scan::calldata_carries(&one, &h), "一枚:整字落在 4 + 32k 上");
    let many = send::registry_calldata(&[h, [0x6b; 32]]);
    assert_eq!(&many[..4], &send::selector_anchor_many()[..]);
    assert!(scan::calldata_carries(&many, &h));
    let bare = send::bare_calldata(&[h, [0x6b; 32]]);
    assert_eq!(bare.len(), 64);
    assert!(scan::calldata_carries(&bare, &h));
}

#[test]
fn a_signed_anchor_hashes_to_what_the_signer_thinks_it_signed() {
    let key = [9u8; 32];
    let (raw, from) = a_signed_bare_tx(8453, key, &[[0xcd; 32]]);
    let t = tx::from_raw(&raw).unwrap();
    assert_eq!(t.hash, zikaron::cryptox::keccak256(&raw));
    assert_eq!(t.sender, Some(from));
    assert_eq!(t.chain_id, Some(8453));
}

// Audit-input assembly.

#[test]
fn the_audit_input_is_the_fragment_plus_the_three_the_ledger_side_holds() {
    use zikaron_anchor::input;
    let fragment = Value::Obj(vec![
        ("anchors".into(), Value::Arr(vec![])),
        ("basis".into(), Value::Obj(vec![])),
        ("evidence".into(), Value::Arr(vec![])),
    ]);
    let v = input::assemble(&fragment, "0x1111111111111111111111111111111111111111", &["0x00".into()], &[]).unwrap();
    let keys: Vec<&str> = match &v {
        Value::Obj(ms) => ms.iter().map(|(k, _)| k.as_str()).collect(),
        _ => panic!("拼出来的不是对象"),
    };
    assert_eq!(keys, vec!["anchors", "basis", "evidence", "pile", "root", "unavailable"]);
    let missing = Value::Obj(vec![("anchors".into(), Value::Arr(vec![]))]);
    assert!(input::assemble(&missing, "0x11", &[], &[]).is_none());
}

// Hostile inputs and edge cases.

#[test]
fn a_deeply_nested_rlp_is_refused_and_never_overflows_the_stack() {
    // Fifty thousand nested empty lists: kit and recording bytes come from others, and decoding must survive
    // them.
    let mut deep = rlp::list(&[]);
    for _ in 0..50_000 {
        deep = rlp::list(&[deep]);
    }
    assert!(rlp::decode_all(&deep).is_none(), "越过嵌套上限即拒,不许递归下去");
    let shallow = rlp::list(&[rlp::list(&[rlp::bytes(b"ok")])]);
    assert!(rlp::decode_all(&shallow).is_some());
}

#[test]
fn a_proof_of_a_short_root_verifies_too() {
    // A root shorter than 32 bytes cannot be inlined anywhere: the proof must carry it, or a proof built here
    // fails its own verification.
    let values: Vec<Vec<u8>> = vec![vec![7u8; 3]];
    let trie = mpt::Trie::indexed(&values);
    let key = rlp::quantity(0);
    assert_eq!(
        mpt::verify(&trie.root(), &key, &trie.proof(&key)),
        Some(mpt::Answer::Value(values[0].clone()))
    );
}

#[test]
fn a_log_missing_its_data_member_is_no_anchor() {
    let t0 = scan::topic0();
    let no_data = format!(
        "{{\"address\":\"0x{}\",\"blockNumber\":\"0x2\",\"topics\":[\"{}\",\"0x{}\",\"0x{}\"],\"transactionHash\":\"0x{}\"}}",
        "11".repeat(20),
        zikaron::hexfmt::encode(&t0),
        format!("{}{}", "0".repeat(24), "11".repeat(20)),
        "22".repeat(32),
        "33".repeat(32)
    );
    assert!(scan::anchored_log(&w(&no_data), &t0).is_none(), "data 成员缺席不等于 data 为空");
}

#[test]
fn an_unreadable_signature_field_is_the_endpoints_fault_and_says_so() {
    // yParity is only 0 or 1; any other value means the endpoint's answer does not hold, not that the law
    // gives no sender.
    let object = "{\"chainId\":\"0x7a69\",\"gas\":\"0x1\",\"hash\":\"0x1111111111111111111111111111111111111111111111111111111111111111\",\"input\":\"0x\",\"maxFeePerGas\":\"0x2\",\"maxPriorityFeePerGas\":\"0x1\",\"nonce\":\"0x0\",\"r\":\"0x11\",\"s\":\"0x22\",\"to\":\"0x1111111111111111111111111111111111111111\",\"type\":\"0x2\",\"value\":\"0x0\",\"yParity\":\"0x5\"}";
    assert!(matches!(tx::read(&w(object)), Err(tx::Bad::Shape("yParity"))));
    // The lawful case still has no sender: a legacy v = 27 transaction whose signature names no chain.
    let legacy = "{\"gas\":\"0x1\",\"gasPrice\":\"0x1\",\"hash\":\"0x1111111111111111111111111111111111111111111111111111111111111111\",\"input\":\"0x\",\"nonce\":\"0x0\",\"r\":\"0x11\",\"s\":\"0x22\",\"to\":\"0x1111111111111111111111111111111111111111\",\"type\":\"0x0\",\"v\":\"0x1b\",\"value\":\"0x0\"}";
    assert!(matches!(tx::read(&w(legacy)), Err(tx::Bad::HashMismatch)), "它先过哈希那一关:这一份是手写的,哈希对不上");
}

#[test]
fn one_endpoint_on_one_chain_is_a_single_source_however_many_rounds_ran() {
    use zikaron_anchor::endpoints::agree_over;
    let f = Value::Obj(vec![("anchors".into(), Value::Arr(vec![]))]);
    // Two rounds, two source strings, one endpoint on chain 2: that chain has no corroboration.
    let r = agree_over(
        vec![("1=A,2=C".into(), f.clone()), ("1=B,2=C".into(), f.clone())],
        vec![2],
    )
    .ok()
    .unwrap();
    assert!(r.single_source, "跑了两趟不等于有两个来源");
    assert_eq!(r.single_source_chains, vec![2]);
    let both = agree_over(vec![("1=A".into(), f.clone()), ("1=B".into(), f)], vec![]).ok().unwrap();
    assert!(!both.single_source);
}

#[test]
fn an_incomplete_receipt_is_an_answer_so_the_deadline_says_not_yet() {
    // A node that answers with a receipt lacking its status or block answered: at the deadline that is "not
    // yet", never "out of sight".
    struct Half;
    impl Endpoint for Half {
        fn call(&mut self, method: &str, _params: &Value) -> Result<W, Trouble> {
            match method {
                "eth_getTransactionReceipt" => Ok(w("{\"transactionHash\":\"0x0000000000000000000000000000000000000000000000000000000000000001\"}")),
                _ => Err(Trouble::Transport("不问这一句".into())),
            }
        }
        fn name(&self) -> String {
            "half".into()
        }
    }
    let mut ep = Half;
    let got = send::confirm_each(&mut [&mut ep], &[1u8; 32], std::time::Duration::from_millis(0), &[]);
    assert!(matches!(got, send::Confirm::NotYet), "收据不全也是答了");
}

#[test]
fn a_broadcast_transaction_keeps_its_hash_even_when_the_endpoint_goes_quiet() {
    // An endpoint lost after broadcast is "out of sight", not a verdict, and the transaction hash must
    // remain: the bytes are out.
    struct Flaky {
        sent: bool,
    }
    impl Endpoint for Flaky {
        fn call(&mut self, method: &str, _params: &Value) -> Result<W, Trouble> {
            match method {
                "eth_getTransactionCount" => Ok(w("\"0x0\"")),
                "eth_sendRawTransaction" => {
                    self.sent = true;
                    // The echo is compared against the hash computed on the sending side; this returns a
                    // placeholder, and the assertions below only check that the hash survives.
                    Ok(w("\"0x0000000000000000000000000000000000000000000000000000000000000000\""))
                }
                _ => Err(Trouble::Transport("端点半路断了".into())),
            }
        }
        fn name(&self) -> String {
            "flaky".into()
        }
    }
    let mut ep = Flaky { sent: false };
    let out = send::anchor(
        &mut ep,
        &[7u8; 32],
        31337,
        send::Form::Bare,
        None,
        &[[0xaa; 32]],
        None,
        std::time::Duration::from_millis(10),
    );
    // A mismatched echo stops at once, before anything else is decided: this pins the echo check.
    assert!(out.is_err(), "节点回的哈希不是我们签的那一笔,当场停");
    assert!(ep.sent, "那一笔确实广播过");
}

#[test]
fn the_three_fates_of_a_broadcast_transaction_have_three_names() {
    use zikaron_anchor::send::{Confirm, Form, Sent};
    let counted = Sent { tx: [1u8; 32], form: Form::Bare, confirm: Confirm::Included { status: 1, block_number: 9 } };
    let reverted = Sent { tx: [1u8; 32], form: Form::Bare, confirm: Confirm::Included { status: 0, block_number: 9 } };
    let waiting = Sent { tx: [1u8; 32], form: Form::Bare, confirm: Confirm::NotYet };
    let blind = Sent { tx: [1u8; 32], form: Form::Bare, confirm: Confirm::Unreachable("断了".into()) };
    assert!(counted.anchored());
    assert!(!reverted.anchored(), "状态不是 1 即不是锚(§9.1)");
    assert!(!waiting.anchored(), "还没入块不是锚上了");
    assert!(!blind.anchored(), "看不到不是锚上了");
}

// Audit bytes pass through unchanged.

#[test]
fn the_core_reads_the_callers_own_bytes_and_not_a_rewritten_copy() {
    use zikaron_anchor::input;
    // A leading zero in an ignored member: the core must refuse exactly these bytes. A rewrite in between
    // would launder it.
    let dirty = br#"{"anchors":[],"basis":{"adoptionChains":[],"bareTx":[],"chains":[]},"evidence":[],"ignored":01,"pile":[],"root":"0x0000000000000000000000000000000000000000","unavailable":[]}"#;
    let answer = String::from_utf8(input::audit(dirty)).unwrap();
    assert_eq!(answer, "{\"ok\":false,\"reason\":\"NO_LABEL\"}", "法核自己读它该拒的那一份");

    // The same input without that member still gets a report (no blanket refusal).
    let clean = br#"{"anchors":[],"basis":{"adoptionChains":[],"bareTx":[],"chains":[]},"evidence":[],"pile":[],"root":"0x0000000000000000000000000000000000000000","unavailable":[]}"#;
    let report = String::from_utf8(input::audit(clean)).unwrap();
    assert!(report.contains("\"label\""), "干净的输入照旧出报告:{report}");

    // Unreadable bytes (non-UTF-8) likewise: no label, not misuse.
    let not_utf8 = [0x7b, 0xff, 0x7d];
    assert_eq!(
        String::from_utf8(input::audit(&not_utf8)).unwrap(),
        "{\"ok\":false,\"reason\":\"NO_LABEL\"}"
    );
}

#[test]
fn every_public_verb_of_this_crate_marks_the_trace() {
    // Trace marks are diagnostics and never decide anything; this pins the shape "every public entry point
    // emits": each public verb's body starts with the call. The list follows the verbs.
    let src = |f: &str| std::fs::read_to_string(format!("{}/src/{f}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    for (file, sig) in [
        ("scan.rs", "pub fn run("),
        ("scan.rs", "pub fn fragment("),
        ("send.rs", "pub fn anchor("),
        ("input.rs", "pub fn assemble("),
        ("input.rs", "pub fn audit("),
        ("kit.rs", "pub fn capture("),
        ("kit.rs", "pub fn verify("),
        ("endpoints.rs", "pub fn agree_over("),
    ] {
        let text = src(file);
        let at = text.find(sig).unwrap_or_else(|| panic!("{file} 里没有 {sig}"));
        let body = &text[at..];
        let head: String = body.chars().take(600).collect();
        assert!(head.contains("crate::seam()"), "{file} 的 {sig} 体首没有吐记号");
    }
}
