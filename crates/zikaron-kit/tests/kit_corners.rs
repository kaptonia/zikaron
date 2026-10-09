//! Edge-case tests for kit law §3 to §10. Samples are built and signed inside the tests; nothing is copied
//! from `base/`.

use zikaron::cryptox;
use zikaron::entry;
use zikaron::hexfmt;
use zikaron::json::{self, Value};
use zikaron_kit::b64;
use zikaron_kit::badge;
use zikaron_kit::check::{self, Hop};
use zikaron_kit::doc::{self, Pairing};
use zikaron_kit::kitdir::{self, KitVerdict};
use zikaron_kit::reading;

const KEY_A: [u8; 32] = [0x11; 32];
const KEY_B: [u8; 32] = [0x22; 32];
const KEY_C: [u8; 32] = [0x33; 32];

fn addr(k: &[u8; 32]) -> String {
    hexfmt::encode(&cryptox::address_of_privkey(k).unwrap())
}
fn o(p: Vec<(&str, Value)>) -> Value {
    Value::Obj(p.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}
fn st(x: &str) -> Value {
    Value::Str(x.to_string())
}
fn h32(x: u8) -> String {
    hexfmt::encode(&[x; 32])
}

/// Sign a document object without `sig`, add the `sig`, return canonical bytes.
fn sign_doc(key: &[u8; 32], body: Value, domain: &str) -> Vec<u8> {
    let pre = json::canon_bytes(&body);
    let (_, digest) = entry::presig_and_digest(&pre, domain);
    let (r, s, v) = cryptox::sign_digest(key, &digest).unwrap();
    let mut sig = Vec::new();
    sig.extend_from_slice(&r);
    sig.extend_from_slice(&s);
    sig.push(v);
    let mut ms = match body {
        Value::Obj(m) => m,
        _ => unreachable!(),
    };
    ms.push(("sig".to_string(), st(&hexfmt::encode(&sig))));
    json::canon_bytes(&Value::Obj(ms))
}

fn fpm_of(key: &[u8; 32], rows: Vec<(String, String)>) -> Vec<u8> {
    let body = o(vec![
        ("spec", st("zikaron.fpm/1")),
        ("author", st(&addr(key))),
        ("work", st(&h32(0x01))),
        ("grant", Value::Null),
        (
            "rows",
            Value::Arr(
                rows.iter()
                    .map(|(r, v)| o(vec![("recipient", st(r)), ("variant", st(v))]))
                    .collect(),
            ),
        ),
        ("note_md", st("")),
    ]);
    sign_doc(key, body, "zikaron.fpm/1")
}

fn ack_of(key: &[u8; 32], fpm_id: &str, variant: &str) -> Vec<u8> {
    let body = o(vec![
        ("spec", st("zikaron.ack/1")),
        ("recipient", st(&addr(key))),
        ("fpm", st(fpm_id)),
        ("variant", st(variant)),
        ("note_md", st("")),
    ]);
    sign_doc(key, body, "zikaron.ack/1")
}

/// Build a signed zikaron/1 entry.
fn entry_of(key: &[u8; 32], ty: &str, seq: u64, prev: Option<&str>, body: Value) -> Vec<u8> {
    let six = o(vec![
        ("spec", st("zikaron/1")),
        ("entryType", st(ty)),
        ("author", st(&addr(key))),
        ("seq", Value::Int(seq)),
        (
            "prev",
            match prev {
                Some(p) => st(p),
                None => Value::Null,
            },
        ),
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

fn id_of(b: &[u8]) -> String {
    hexfmt::encode(&entry::entry_id(b))
}

// §3 to §5: documents.

#[test]
fn a_self_signed_manifest_is_a_manifest() {
    let b = fpm_of(&KEY_A, vec![(addr(&KEY_B), h32(0xaa))]);
    let f = doc::check_fpm(&b).expect("manifest");
    assert_eq!(f.author, addr(&KEY_A));
    assert_eq!(f.rows.len(), 1);
}

#[test]
fn manifest_decision_order() {
    let b = fpm_of(&KEY_A, vec![(addr(&KEY_B), h32(0xaa))]);
    let s = String::from_utf8(b.clone()).unwrap();
    // The root is not an object.
    assert_eq!(doc::check_fpm(b"[]").unwrap_err().token.as_str(), "E_DOC");
    // An eighth key.
    let closed = s.replace("{\"author\"", "{\"aextra\":1,\"author\"");
    assert_eq!(doc::check_fpm(closed.as_bytes()).unwrap_err().token.as_str(), "E_DOC_CLOSED");
    // Wrong spec.
    let spec = s.replace("\"zikaron.fpm/1\"", "\"zikaron.fpm/2\"");
    assert_eq!(doc::check_fpm(spec.as_bytes()).unwrap_err().token.as_str(), "E_SPEC");
    // A missing key.
    let missing = json::canon_bytes(&o(vec![("spec", st("zikaron.fpm/1"))]));
    assert_eq!(doc::check_fpm(&missing).unwrap_err().token.as_str(), "E_DOC_MISSING");
}

#[test]
fn manifest_row_rules_carry_the_index() {
    // An extra member in a row: E_FPM_ROW with its index.
    let body = o(vec![
        ("spec", st("zikaron.fpm/1")),
        ("author", st(&addr(&KEY_A))),
        ("work", st(&h32(0x01))),
        ("grant", Value::Null),
        (
            "rows",
            Value::Arr(vec![
                o(vec![("recipient", st(&addr(&KEY_B))), ("variant", st(&h32(0xaa)))]),
                o(vec![
                    ("recipient", st(&addr(&KEY_C))),
                    ("variant", st(&h32(0xbb))),
                    ("extra", Value::Int(1)),
                ]),
            ]),
        ),
        ("note_md", st("")),
    ]);
    let b = sign_doc(&KEY_A, body, "zikaron.fpm/1");
    let e = doc::check_fpm(&b).unwrap_err();
    assert_eq!(e.token.as_str(), "E_FPM_ROW");
    assert_eq!(e.index, Some(1));
}

#[test]
fn manifest_rows_must_be_distinct_and_sorted() {
    let a = addr(&KEY_B);
    let c = addr(&KEY_C);
    let (lo, hi) = if a.as_bytes() < c.as_bytes() { (a.clone(), c.clone()) } else { (c.clone(), a.clone()) };
    // Duplicate recipient.
    let b = fpm_of(&KEY_A, vec![(lo.clone(), h32(0xaa)), (lo.clone(), h32(0xbb))]);
    assert_eq!(doc::check_fpm(&b).unwrap_err().token.as_str(), "E_FPM_DUP_RECIPIENT");
    // Duplicate variant.
    let b = fpm_of(&KEY_A, vec![(lo.clone(), h32(0xaa)), (hi.clone(), h32(0xaa))]);
    assert_eq!(doc::check_fpm(&b).unwrap_err().token.as_str(), "E_FPM_DUP_VARIANT");
    // Rows out of order.
    let b = fpm_of(&KEY_A, vec![(hi.clone(), h32(0xaa)), (lo.clone(), h32(0xbb))]);
    assert_eq!(doc::check_fpm(&b).unwrap_err().token.as_str(), "E_FPM_ROW_ORDER");
    // Two rows in order.
    let b = fpm_of(&KEY_A, vec![(lo, h32(0xaa)), (hi, h32(0xbb))]);
    assert!(doc::check_fpm(&b).is_ok());
}

#[test]
fn a_manifest_signed_by_another_key_is_refused() {
    let body = o(vec![
        ("spec", st("zikaron.fpm/1")),
        ("author", st(&addr(&KEY_A))),
        ("work", st(&h32(0x01))),
        ("grant", Value::Null),
        (
            "rows",
            Value::Arr(vec![o(vec![
                ("recipient", st(&addr(&KEY_B))),
                ("variant", st(&h32(0xaa))),
            ])]),
        ),
        ("note_md", st("")),
    ]);
    let b = sign_doc(&KEY_B, body, "zikaron.fpm/1");
    assert_eq!(doc::check_fpm(&b).unwrap_err().token.as_str(), "E_SIG_SIGNER");
}

#[test]
fn a_manifest_signed_under_the_ack_domain_is_refused() {
    let body = o(vec![
        ("spec", st("zikaron.fpm/1")),
        ("author", st(&addr(&KEY_A))),
        ("work", st(&h32(0x01))),
        ("grant", Value::Null),
        (
            "rows",
            Value::Arr(vec![o(vec![
                ("recipient", st(&addr(&KEY_B))),
                ("variant", st(&h32(0xaa))),
            ])]),
        ),
        ("note_md", st("")),
    ]);
    let b = sign_doc(&KEY_A, body, "zikaron.ack/1");
    // The two domains give different messages: a signature does not hold under the other domain.
    assert!(doc::check_fpm(&b).unwrap_err().token.as_str().starts_with("E_SIG"));
}

#[test]
fn pairing_walks_its_six_verdicts() {
    let m = fpm_of(&KEY_A, vec![(addr(&KEY_B), h32(0xaa))]);
    let mid = id_of(&m);
    // PAIRED.
    let a = ack_of(&KEY_B, &mid, &h32(0xaa));
    match doc::pair(&m, &a) {
        Pairing::Paired { recipient, variant } => {
            assert_eq!(recipient, addr(&KEY_B));
            assert_eq!(variant, h32(0xaa));
        }
        other => panic!("{:?}", other),
    }
    // Variant mismatch.
    let a = ack_of(&KEY_B, &mid, &h32(0xbb));
    assert_eq!(doc::pair(&m, &a).verdict().as_str(), "ACK_VARIANT_MISMATCH");
    // No row for this recipient.
    let a = ack_of(&KEY_C, &mid, &h32(0xaa));
    assert_eq!(doc::pair(&m, &a).verdict().as_str(), "ACK_NO_ROW");
    // Points to another manifest.
    let a = ack_of(&KEY_B, &h32(0x99), &h32(0xaa));
    assert_eq!(doc::pair(&m, &a).verdict().as_str(), "ACK_FPM_MISMATCH");
    // Broken manifest.
    let a = ack_of(&KEY_B, &mid, &h32(0xaa));
    assert_eq!(doc::pair(b"[]", &a).verdict().as_str(), "FPM_INVALID");
    // Broken acknowledgement.
    assert_eq!(doc::pair(&m, b"[]").verdict().as_str(), "ACK_INVALID");
}

#[test]
fn attribution_is_a_statement_about_bytes() {
    let x = b"the bytes that went out";
    let variant = hexfmt::encode(&entry::entry_id(x));
    let m = fpm_of(&KEY_A, vec![(addr(&KEY_B), variant.clone())]);
    let a = ack_of(&KEY_B, &id_of(&m), &variant);
    assert_eq!(doc::attribute(&m, &a, x).1, Some(true));
    assert_eq!(doc::attribute(&m, &a, b"other bytes").1, Some(false));
}

// §6 payloads.

#[test]
fn base64url_is_canonical_and_unpadded() {
    for n in 0..40usize {
        let bytes: Vec<u8> = (0..n).map(|i| (i * 7 + 3) as u8).collect();
        let enc = b64::encode(&bytes);
        assert!(!enc.contains('='));
        assert_eq!(b64::decode(enc.as_bytes()).as_deref(), if n == 0 { None } else { Some(&bytes[..]) });
    }
    // Nonzero unused bits in the last character are refused.
    assert_eq!(b64::decode(b"AA"), Some(vec![0u8]));
    assert!(b64::decode(b"AB").is_none());
    // Length ≡ 1 (mod 4) is refused.
    assert!(b64::decode(b"A").is_none());
    // Bytes outside the alphabet are refused.
    assert!(b64::decode(b"A+B/").is_none());
}

fn grant_entry(key: &[u8; 32], seq: u64, prev: Option<&str>, work: &str, upstream: Option<&str>, grantee: &str) -> Vec<u8> {
    let mut body = vec![
        ("grantee", st(grantee)),
        ("work", st(work)),
        ("terms", st(&h32(0x02))),
    ];
    if let Some(u) = upstream {
        body.push(("upstream", st(u)));
    }
    entry_of(key, "grant", seq, prev, o(body))
}

#[test]
fn an_empty_badge_is_refused_on_both_sides() {
    // §6.1 is defined for one or more segments; with zero the encoder outputs the prefix alone (none of its
    // three tokens covers it) and §6.2 refuses it at segment 0 with E_BADGE_B64. The harness always passes at
    // least one path, so this case is covered here.
    let payload = badge::encode(&[]).unwrap();
    assert_eq!(payload, "zikaron-grant:");
    let e = badge::decode(payload.as_bytes()).unwrap_err();
    assert_eq!(e.token.as_str(), "E_BADGE_B64");
    assert_eq!(e.index, Some(0));
}

#[test]
fn reachability_starts_at_an_entry_the_ledger_need_not_hold() {
    // §8.1 only asks that each step along prev be a ledger entry; the start may be outside the ledger (for
    // example an entry trimmed to EXCLUDED by the §8.1 lineage).
    let g = entry_of(&KEY_A, "genesis", 0, None, o(vec![("statement_md", st("l"))]));
    let head = id_of(&g);
    let a = entry_of(&KEY_A, "annotation", 1, Some(&head), o(vec![("note_md", st("in"))]));
    let ledger_input = audit_input(&addr(&KEY_A), vec![&g, &a], Value::Arr(vec![]), basis_of(vec![], false));
    let outcome = zikaron::audit::audit_full(&ledger_input).unwrap();
    // An entry outside the ledger: signed by someone else, prev pointing at the ledger head.
    let outsider_bytes = entry_of(&KEY_B, "annotation", 2, Some(&id_of(&a)), o(vec![("note_md", st("out"))]));
    let outsider = zikaron::entry::check(&outsider_bytes).unwrap();
    assert!(reading::reachable(&outcome.ledger, &outsider, &head));
    assert!(!reading::reachable(&outcome.ledger, &outsider, &h32(0x99)));
}

#[test]
fn a_badge_of_one_grant_round_trips() {
    let g = entry_of(&KEY_A, "genesis", 0, None, o(vec![("statement_md", st("l"))]));
    let head = id_of(&g);
    let grant = grant_entry(&KEY_A, 1, Some(&head), &h32(0x77), None, &addr(&KEY_B));
    let payload = badge::encode(&[grant.clone()]).unwrap();
    assert!(payload.starts_with("zikaron-grant:"));
    let back = badge::decode(payload.as_bytes()).unwrap();
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].id_hex(), id_of(&grant));
}

#[test]
fn a_badge_that_starts_with_an_upstream_is_incomplete() {
    let g = entry_of(&KEY_A, "genesis", 0, None, o(vec![("statement_md", st("l"))]));
    let head = id_of(&g);
    let grant = grant_entry(&KEY_A, 1, Some(&head), &h32(0x77), Some(&h32(0x66)), &addr(&KEY_B));
    let payload = badge::encode(&[grant]).unwrap();
    assert_eq!(badge::decode(payload.as_bytes()).unwrap_err().token.as_str(), "E_BADGE_INCOMPLETE");
}

#[test]
fn a_broken_byte_link_names_its_segment() {
    let g = entry_of(&KEY_A, "genesis", 0, None, o(vec![("statement_md", st("l"))]));
    let head = id_of(&g);
    let up = grant_entry(&KEY_A, 1, Some(&head), &h32(0x77), None, &addr(&KEY_B));
    // The downstream declares a different upstream entry_id.
    let g2 = entry_of(&KEY_B, "genesis", 0, None, o(vec![("statement_md", st("l2"))]));
    let head2 = id_of(&g2);
    let down = grant_entry(&KEY_B, 1, Some(&head2), &h32(0x77), Some(&h32(0x55)), &addr(&KEY_C));
    let payload = badge::encode(&[up.clone(), down]).unwrap();
    let e = badge::decode(payload.as_bytes()).unwrap_err();
    assert_eq!(e.token.as_str(), "E_BADGE_LINK");
    assert_eq!(e.index, Some(1));
    // Right upstream, right work: the link holds.
    let down_ok = grant_entry(&KEY_B, 1, Some(&head2), &h32(0x77), Some(&id_of(&up)), &addr(&KEY_C));
    let payload = badge::encode(&[up, down_ok]).unwrap();
    assert_eq!(badge::decode(payload.as_bytes()).unwrap().len(), 2);
}

#[test]
fn a_badge_without_the_prefix_is_refused() {
    assert_eq!(badge::decode(b"grant:abc").unwrap_err().token.as_str(), "E_BADGE_PREFIX");
}

#[test]
fn a_badge_segment_that_is_no_entry_carries_the_parent_token() {
    let payload = format!("zikaron-grant:{}", b64::encode(b"[]"));
    let e = badge::decode(payload.as_bytes()).unwrap_err();
    assert_eq!(e.token.as_str(), "E_BADGE_ENTRY");
    assert_eq!(e.index, Some(0));
    assert_eq!(e.inner.map(|t| t.as_str()), Some("E_ENVELOPE"));
}

// §7 disclosure kits.

fn tmpdir(name: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let p = std::env::temp_dir().join(format!("zkk-test-{name}-{nanos}"));
    std::fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn kit_paths_follow_7_2() {
    assert!(kitdir::is_kit_path("a"));
    assert!(kitdir::is_kit_path("a/b-c.d_e"));
    assert!(!kitdir::is_kit_path(""));
    assert!(!kitdir::is_kit_path("/a"));
    assert!(!kitdir::is_kit_path("a//b"));
    assert!(!kitdir::is_kit_path(".."));
    assert!(!kitdir::is_kit_path("a/../b"));
    assert!(!kitdir::is_kit_path("-a"));
    assert!(!kitdir::is_kit_path("A"));
    assert!(!kitdir::is_kit_path("a b"));
}

#[test]
fn an_empty_kit_is_ok_and_says_so_through_its_counts() {
    let dir = tmpdir("empty");
    let manifest = json::canon_bytes(&o(vec![
        ("spec", st("zikaron.kit/1")),
        ("root", Value::Null),
        ("entries", Value::Arr(vec![])),
        ("files", Value::Arr(vec![])),
        ("contents", Value::Arr(vec![])),
        ("proofs", Value::Arr(vec![])),
        ("note_md", st("")),
    ]));
    std::fs::write(dir.join("manifest.json"), &manifest).unwrap();
    match kitdir::verify_kit(&dir) {
        KitVerdict::Ok { entries, files, proofs, invalid, kit_id } => {
            assert_eq!((entries, files, proofs), (0, 0, 0));
            assert!(invalid.is_empty());
            assert_eq!(hexfmt::encode(&kit_id), hexfmt::encode(&entry::entry_id(&manifest)));
        }
        other => panic!("{:?}", other),
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_kit_without_a_manifest_and_a_kit_with_an_extra_file() {
    let dir = tmpdir("noman");
    std::fs::write(dir.join("stray.txt"), b"x").unwrap();
    match kitdir::verify_kit(&dir) {
        KitVerdict::Fail { verdict, subject } => {
            assert_eq!(verdict.as_str(), "E_KIT_MANIFEST_ABSENT");
            assert!(subject.is_none());
        }
        other => panic!("{:?}", other),
    }
    let manifest = json::canon_bytes(&o(vec![
        ("spec", st("zikaron.kit/1")),
        ("root", Value::Null),
        ("entries", Value::Arr(vec![])),
        ("files", Value::Arr(vec![])),
        ("contents", Value::Arr(vec![])),
        ("proofs", Value::Arr(vec![])),
        ("note_md", st("")),
    ]));
    std::fs::write(dir.join("manifest.json"), &manifest).unwrap();
    match kitdir::verify_kit(&dir) {
        KitVerdict::Fail { verdict, subject } => {
            assert_eq!(verdict.as_str(), "E_KIT_EXTRA");
            assert_eq!(subject.as_deref(), Some("stray.txt"));
        }
        other => panic!("{:?}", other),
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_kit_carrying_an_entry_reports_its_bytes_and_its_invalids() {
    let dir = tmpdir("entries");
    std::fs::create_dir_all(dir.join("entries")).unwrap();
    let g = entry_of(&KEY_A, "genesis", 0, None, o(vec![("statement_md", st("l"))]));
    let gid = id_of(&g);
    let junk = b"not an entry".to_vec();
    let jid = hexfmt::encode(&entry::entry_id(&junk));
    std::fs::write(dir.join(format!("entries/{}.zk1", &gid[2..])), &g).unwrap();
    std::fs::write(dir.join(format!("entries/{}.zk1", &jid[2..])), &junk).unwrap();
    let mut ids = vec![gid.clone(), jid.clone()];
    ids.sort();
    let manifest = json::canon_bytes(&o(vec![
        ("spec", st("zikaron.kit/1")),
        ("root", st(&addr(&KEY_A))),
        ("entries", Value::Arr(ids.iter().map(|x| st(x)).collect())),
        ("files", Value::Arr(vec![])),
        ("contents", Value::Arr(vec![])),
        ("proofs", Value::Arr(vec![])),
        ("note_md", st("")),
    ]));
    std::fs::write(dir.join("manifest.json"), &manifest).unwrap();
    match kitdir::verify_kit(&dir) {
        KitVerdict::Ok { entries, invalid, .. } => {
            assert_eq!(entries, 2);
            assert_eq!(invalid.len(), 1);
            assert_eq!(invalid[0].0, jid);
        }
        other => panic!("{:?}", other),
    }
    std::fs::remove_dir_all(&dir).ok();
}

// §9 and §10 readings.

fn audit_input(root: &str, pile: Vec<&Vec<u8>>, anchors: Value, basis: Value) -> Value {
    o(vec![
        ("root", st(root)),
        (
            "pile",
            Value::Arr(pile.iter().map(|b| st(&hexfmt::encode(b))).collect()),
        ),
        ("anchors", anchors),
        ("unavailable", Value::Arr(vec![])),
        ("evidence", Value::Arr(vec![])),
        ("basis", basis),
    ])
}

fn basis_of(senders: Vec<String>, registries: bool) -> Value {
    o(vec![
        (
            "chains",
            Value::Arr(vec![o(vec![
                ("chainId", Value::Int(1)),
                ("fromBlock", Value::Int(0)),
                ("toBlock", Value::Int(100)),
                (
                    "registries",
                    Value::Arr(if registries { vec![st(&addr(&KEY_C))] } else { vec![] }),
                ),
                ("senders", Value::Arr(senders.iter().map(|x| st(x)).collect())),
            ])]),
        ),
        ("bareTx", Value::Arr(vec![])),
        ("adoptionChains", Value::Arr(vec![])),
    ])
}

fn anchor_of(sender: &str, hash: &str, ts: u64) -> Value {
    o(vec![
        ("chainId", Value::Int(1)),
        ("blockNumber", Value::Int(10)),
        ("blockTimestamp", Value::Int(ts)),
        ("tx", st(&h32(0x31))),
        ("sender", st(sender)),
        ("hash", st(hash)),
        ("verdict", st("counted")),
    ])
}

#[test]
fn depth_reads_history_entries_of_one_digest() {
    let g = entry_of(&KEY_A, "genesis", 0, None, o(vec![("statement_md", st("l"))]));
    let head = id_of(&g);
    let mode = o(vec![("mark", st("hand")), ("toolchain", st(&h32(0x05)))]);
    let h1 = entry_of(
        &KEY_A,
        "history",
        1,
        Some(&head),
        o(vec![("content", st(&h32(0x77))), ("mode", mode.clone())]),
    );
    let h1id = id_of(&h1);
    let h2 = entry_of(
        &KEY_A,
        "history",
        2,
        Some(&h1id),
        o(vec![("content", st(&h32(0x77))), ("mode", mode)]),
    );
    let input = audit_input(
        &addr(&KEY_A),
        vec![&g, &h1, &h2],
        Value::Arr(vec![anchor_of(&addr(&KEY_A), &id_of(&h2), 1700)]),
        basis_of(vec![addr(&KEY_A)], true),
    );
    let outcome = zikaron::audit::audit_full(&input).unwrap();
    let r = reading::depth(Some(&outcome), &h32(0x77));
    assert_eq!(r.member("found").unwrap(), &Value::Bool(true));
    // The anchor on h2 bounds h1 too, walking back along prev: both are anchored.
    assert_eq!(r.member("deepest").unwrap().as_int().unwrap(), 2);
    assert_eq!(r.member("earliest").unwrap().as_int().unwrap(), 1700);
    let c = r.member("continuity").unwrap();
    assert_eq!(c.member("span").unwrap().as_int().unwrap(), 2);
    // Only the anchor on h2 sits directly on H0's line.
    assert_eq!(c.member("anchored").unwrap().as_int().unwrap(), 1);
    // Another work digest: not found.
    let r = reading::depth(Some(&outcome), &h32(0x66));
    assert_eq!(r.member("found").unwrap(), &Value::Bool(false));
    assert_eq!(r.member("deepest").unwrap().as_int().unwrap(), 0);
    // An invalid input yields only valid false.
    let r = reading::depth(None, &h32(0x77));
    assert_eq!(r, o(vec![("valid", Value::Bool(false))]));
}

#[test]
fn a_grant_with_no_audit_input_is_partial_at_best() {
    let g = entry_of(&KEY_A, "genesis", 0, None, o(vec![("statement_md", st("l"))]));
    let head = id_of(&g);
    let grant = grant_entry(&KEY_A, 1, Some(&head), &h32(0x77), None, &addr(&KEY_B));
    let r = check::grant_check(&grant, None, None);
    assert_eq!(r.verdict.as_str(), "PARTIAL");
    let checks = r.value.member("checks").unwrap().as_arr().unwrap();
    assert_eq!(checks[0].member("state").unwrap().as_str().unwrap(), "PASS");
    assert_eq!(checks[1].member("state").unwrap().as_str().unwrap(), "UNKNOWN");
    assert_eq!(r.value.member("basis").unwrap(), &Value::Null);
}

#[test]
fn a_grant_that_is_no_entry_fails_check_one_and_leaves_the_rest_unknown() {
    let r = check::grant_check(b"[]", None, None);
    assert_eq!(r.verdict.as_str(), "FAIL");
    let checks = r.value.member("checks").unwrap().as_arr().unwrap();
    assert_eq!(checks[0].member("reason").unwrap().as_str().unwrap(), "E_ENVELOPE");
    for c in checks.iter().skip(1) {
        assert_eq!(c.member("state").unwrap().as_str().unwrap(), "UNKNOWN");
    }
    assert_eq!(
        r.value.member("failed").unwrap().as_arr().unwrap()[0].as_str().unwrap(),
        "BAD_SIG"
    );
}

#[test]
fn an_entry_of_another_type_fails_check_one_as_not_a_grant() {
    let g = entry_of(&KEY_A, "genesis", 0, None, o(vec![("statement_md", st("l"))]));
    let r = check::grant_check(&g, None, None);
    let checks = r.value.member("checks").unwrap().as_arr().unwrap();
    assert_eq!(checks[0].member("reason").unwrap().as_str().unwrap(), "NOT_A_GRANT");
}

#[test]
fn a_green_grant_needs_an_anchor_a_covering_basis_and_a_complete_record() {
    let g = entry_of(&KEY_A, "genesis", 0, None, o(vec![("statement_md", st("l"))]));
    let head = id_of(&g);
    let grant = grant_entry(&KEY_A, 1, Some(&head), &h32(0x77), None, &addr(&KEY_B));
    let gid = id_of(&grant);
    let input = audit_input(
        &addr(&KEY_A),
        vec![&g],
        Value::Arr(vec![
            anchor_of(&addr(&KEY_A), &head, 1600),
            anchor_of(&addr(&KEY_A), &gid, 1700),
        ]),
        basis_of(vec![addr(&KEY_A)], true),
    );
    let r = check::grant_check(&grant, Some(&input), Some(1000));
    assert_eq!(r.verdict.as_str(), "GREEN");
    // A window outside now is FAIL.
    let windowed = {
        let body = o(vec![
            ("grantee", st(&addr(&KEY_B))),
            ("work", st(&h32(0x77))),
            ("terms", st(&h32(0x02))),
            ("window", o(vec![("from", Value::Int(10)), ("to", Value::Int(20))])),
        ]);
        entry_of(&KEY_A, "grant", 1, Some(&head), body)
    };
    let wid = id_of(&windowed);
    let input2 = audit_input(
        &addr(&KEY_A),
        vec![&g],
        Value::Arr(vec![
            anchor_of(&addr(&KEY_A), &head, 1600),
            anchor_of(&addr(&KEY_A), &wid, 1700),
        ]),
        basis_of(vec![addr(&KEY_A)], true),
    );
    let r = check::grant_check(&windowed, Some(&input2), Some(30));
    assert_eq!(r.verdict.as_str(), "FAIL");
    let failed = r.value.member("failed").unwrap().as_arr().unwrap();
    assert_eq!(failed[0].as_str().unwrap(), "EXPIRED");
    // now absent: that check is unknown, the verdict PARTIAL.
    let r = check::grant_check(&windowed, Some(&input2), None);
    assert_eq!(r.verdict.as_str(), "PARTIAL");
}

#[test]
fn a_revocation_in_the_issuers_ledger_fails_check_six() {
    let g = entry_of(&KEY_A, "genesis", 0, None, o(vec![("statement_md", st("l"))]));
    let head = id_of(&g);
    let grant = grant_entry(&KEY_A, 1, Some(&head), &h32(0x77), None, &addr(&KEY_B));
    let gid = id_of(&grant);
    let rev = entry_of(&KEY_A, "revocation", 2, Some(&gid), o(vec![("grant", st(&gid))]));
    let input = audit_input(
        &addr(&KEY_A),
        vec![&g, &rev],
        Value::Arr(vec![
            anchor_of(&addr(&KEY_A), &head, 1600),
            anchor_of(&addr(&KEY_A), &gid, 1700),
        ]),
        basis_of(vec![addr(&KEY_A)], true),
    );
    let r = check::grant_check(&grant, Some(&input), Some(1000));
    assert_eq!(r.verdict.as_str(), "FAIL");
    let failed: Vec<&str> = r
        .value
        .member("failed")
        .unwrap()
        .as_arr()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    assert!(failed.contains(&"REVOKED"));
}

#[test]
fn an_empty_chain_is_fail_with_chain_empty() {
    let v = check::chain_check(&[], None);
    assert_eq!(v.member("verdict").unwrap().as_str().unwrap(), "FAIL");
    assert_eq!(v.member("token").unwrap().as_str().unwrap(), "CHAIN_EMPTY");
    let f = v.member("failing").unwrap();
    assert_eq!(f.member("kind").unwrap().as_str().unwrap(), "empty");
}

#[test]
fn a_chain_whose_first_hop_states_an_upstream_is_incomplete() {
    let g = entry_of(&KEY_A, "genesis", 0, None, o(vec![("statement_md", st("l"))]));
    let head = id_of(&g);
    let grant = grant_entry(&KEY_A, 1, Some(&head), &h32(0x77), Some(&h32(0x66)), &addr(&KEY_B));
    let hops = vec![Hop { grant: &grant, input: None }];
    let v = check::chain_check(&hops, None);
    assert_eq!(v.member("token").unwrap().as_str().unwrap(), "CHAIN_INCOMPLETE");
    assert_eq!(v.member("verdict").unwrap().as_str().unwrap(), "FAIL");
}

#[test]
fn a_two_hop_chain_links_by_bytes_and_by_ledger() {
    let g = entry_of(&KEY_A, "genesis", 0, None, o(vec![("statement_md", st("l"))]));
    let head = id_of(&g);
    let up = grant_entry(&KEY_A, 1, Some(&head), &h32(0x77), None, &addr(&KEY_B));
    let g2 = entry_of(&KEY_B, "genesis", 0, None, o(vec![("statement_md", st("l2"))]));
    let head2 = id_of(&g2);
    let down = grant_entry(&KEY_B, 1, Some(&head2), &h32(0x77), Some(&id_of(&up)), &addr(&KEY_C));
    let input2 = audit_input(&addr(&KEY_B), vec![&g2], Value::Arr(vec![]), basis_of(vec![], false));
    let hops = vec![
        Hop { grant: &up, input: None },
        Hop { grant: &down, input: Some(input2) },
    ];
    let v = check::chain_check(&hops, None);
    let links = v.member("links").unwrap().as_arr().unwrap();
    assert_eq!(links.len(), 1);
    // The downstream ledger's root is the upstream grantee: the link holds.
    assert_eq!(links[0], Value::Bool(true));
    assert_eq!(v.member("failing").unwrap(), &Value::Null);
    assert_eq!(v.member("verdict").unwrap().as_str().unwrap(), "PARTIAL");
}
