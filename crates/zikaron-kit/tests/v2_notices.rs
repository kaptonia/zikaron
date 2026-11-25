use zikaron::cryptox;
use zikaron::entry as v1;
use zikaron::hexfmt;
use zikaron::json::{self, Value};
use zikaron_kit::v2::read::{self, Notices};

const CHAIN: u64 = 7;

fn key(n: u8) -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = n;
    k
}

fn addr(n: u8) -> String {
    hexfmt::encode(&cryptox::address_of_privkey(&key(n)).expect("key"))
}

fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}

fn obj(ms: Vec<(&str, Value)>) -> Value {
    Value::Obj(ms.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn entry(spec: &str, n: u8, etype: &str, seq: u64, prev: Option<&str>, body: Value) -> Vec<u8> {
    let six = obj(vec![
        ("spec", s(spec)),
        ("entryType", s(etype)),
        ("author", s(&addr(n))),
        ("seq", Value::Int(seq)),
        ("prev", prev.map(s).unwrap_or(Value::Null)),
        ("body", body),
    ]);
    let domain = if spec == zikaron::tokens::SPEC { zikaron::tokens::Domain::Entry.as_str() } else { zikaron::v2::tokens::Domain::Entry.as_str() };
    let (_, digest) = v1::presig_and_digest(&json::canon_bytes(&six), domain);
    let (r, sg, v) = cryptox::sign_digest(&key(n), &digest).expect("sign");
    let mut sig = r.to_vec();
    sig.extend_from_slice(&sg);
    sig.push(v);
    let Value::Obj(mut ms) = six else { unreachable!() };
    ms.push(("sig".to_string(), s(&hexfmt::encode(&sig))));
    json::canon_bytes(&Value::Obj(ms))
}

fn id(b: &[u8]) -> String {
    hexfmt::encode(&v1::entry_id(b))
}

fn contract() -> String {
    format!("0x{}", "c0".repeat(20))
}

fn member() -> Value {
    obj(vec![("chain", Value::Int(CHAIN)), ("at", s(&contract()))])
}

fn grant_body(revocation: Option<Value>) -> Value {
    let mut ms = vec![("grantee", s(&addr(9))), ("work", s(&format!("0x{}", "aa".repeat(32)))), ("terms", s(&format!("0x{}", "bb".repeat(32))))];
    if let Some(r) = revocation {
        ms.push(("revocation", r));
    }
    obj(ms)
}

fn genesis(n: u8) -> Vec<u8> {
    entry("zikaron/2", n, "genesis", 0, None, obj(vec![("statement_md", s(""))]))
}

fn ledger(signer: u8, revocation: Option<Value>) -> (Vec<u8>, Vec<u8>) {
    let g1 = genesis(1);
    let grant = entry("zikaron/2", signer, "grant", 1, Some(&id(&g1)), grant_body(revocation));
    (g1, grant)
}

fn anchor(hash: &str, block: u64, sender: u8) -> Value {
    obj(vec![
        ("chainId", Value::Int(CHAIN)),
        ("blockNumber", Value::Int(block)),
        ("blockTimestamp", Value::Int(100 + block)),
        ("tx", s(&format!("0x{:064x}", block))),
        ("sender", s(&addr(sender))),
        ("hash", s(hash)),
        ("verdict", s("counted")),
    ])
}

fn input(root: u8, pile: &[&Vec<u8>], anchors: Vec<Value>) -> Value {
    let mut senders = vec![addr(1), addr(2)];
    senders.sort();
    obj(vec![
        ("root", s(&addr(root))),
        ("pile", Value::Arr(pile.iter().map(|b| s(&hexfmt::encode(b))).collect())),
        ("anchors", Value::Arr(anchors)),
        (
            "basis",
            obj(vec![(
                "chains",
                Value::Arr(vec![obj(vec![
                    ("chainId", Value::Int(CHAIN)),
                    ("fromBlock", Value::Int(0)),
                    ("toBlock", Value::Int(1000)),
                    ("registries", Value::Arr(vec![s(&format!("0x{}", "11".repeat(20)))])),
                    ("senders", Value::Arr(senders.iter().map(|x| s(x)).collect())),
                ])]),
            )]),
        ),
    ])
}

fn nbasis() -> Value {
    obj(vec![
        ("chainId", Value::Int(CHAIN)),
        ("fromBlock", Value::Int(0)),
        ("toBlock", Value::Int(100)),
        ("revocationContract", s(&contract())),
        ("book", s(&format!("0x{}", "b0".repeat(20)))),
        ("bookFromBlock", Value::Int(0)),
    ])
}

fn notice(grant: &str, block: u64, log: u64, sender: u8, verdict: &str) -> Value {
    obj(vec![
        ("chainId", Value::Int(CHAIN)),
        ("blockNumber", Value::Int(block)),
        ("txIndex", Value::Int(0)),
        ("logIndex", Value::Int(log)),
        ("tx", s(&format!("0x{:064x}", 900 + block))),
        ("sender", s(&addr(sender))),
        ("grant", s(grant)),
        ("verdict", s(verdict)),
    ])
}

fn accession(block: u64, log: u64, holder: u8, to: u8, root: u8) -> Value {
    obj(vec![
        ("chainId", Value::Int(CHAIN)),
        ("blockNumber", Value::Int(block)),
        ("txIndex", Value::Int(0)),
        ("logIndex", Value::Int(log)),
        ("holder", s(&addr(holder))),
        ("to", s(&addr(to))),
        ("root", s(&addr(root))),
    ])
}

fn notices(revocations: Vec<Value>, accessions: Vec<Value>) -> Value {
    obj(vec![("basis", nbasis()), ("revocations", Value::Arr(revocations)), ("accessions", Value::Arr(accessions))])
}

fn canon(v: Option<&Value>) -> Vec<u8> {
    v.map(json::canon_bytes).unwrap_or_default()
}

fn loaded(v: &Value) -> Notices {
    read::load_notices(v).expect("a notice input")
}

fn check(grant: &[u8], i: Option<&Value>, n: Option<&Value>) -> Value {
    let view = i.and_then(|i| read::load_view(i, Some(grant)));
    let n = n.and_then(read::load_notices);
    read::grant_check(grant, view.as_ref(), n.as_ref(), None).result
}

fn states(r: &Value) -> Vec<String> {
    r.member("checks").and_then(Value::as_arr).map(|a| a.iter().map(|c| c.member("state").and_then(Value::as_str).unwrap_or("").to_string()).collect()).unwrap_or_default()
}

fn verdict(r: &Value) -> String {
    r.member("verdict").and_then(Value::as_str).unwrap_or("").to_string()
}

#[test]
fn a_notice_by_the_key_in_office_withdraws_and_its_absence_acquits() {
    let (g1, grant) = ledger(1, Some(member()));
    let i = input(1, &[&g1], vec![anchor(&id(&grant), 5, 1)]);
    let silent = check(&grant, Some(&i), Some(&notices(vec![], vec![])));
    assert_eq!(verdict(&silent), "GREEN", "{:?}", states(&silent));
    assert_eq!(canon(silent.member("notices")), canon(Some(&nbasis())));
    let revoked = check(&grant, Some(&i), Some(&notices(vec![notice(&id(&grant), 6, 0, 1, "counted")], vec![])));
    assert_eq!(states(&revoked)[5], "FAIL");
    assert_eq!(verdict(&revoked), "FAIL");
}

#[test]
fn the_result_always_carries_notices_and_a_grant_without_the_member_ignores_them() {
    let (g1, grant) = ledger(1, None);
    let i = input(1, &[&g1], vec![anchor(&id(&grant), 5, 1)]);
    let none = check(&grant, Some(&i), None);
    assert_eq!(none.member("notices"), Some(&Value::Null));
    let given = notices(vec![notice(&id(&grant), 6, 0, 1, "counted")], vec![]);
    let with = check(&grant, Some(&i), Some(&given));
    assert_eq!(canon(with.member("notices")), canon(Some(&nbasis())));
    assert_eq!(states(&with), states(&none), "the ledger decides a grant carrying no member");
    let keys: Vec<&str> = match &with {
        Value::Obj(ms) => ms.iter().map(|(k, _)| k.as_str()).collect(),
        _ => Vec::new(),
    };
    assert_eq!(keys, ["verdict", "basis", "notices", "checks", "failed"]);
}

#[test]
fn an_invalid_notice_input_is_read_as_none() {
    let (g1, grant) = ledger(1, Some(member()));
    let gid = id(&grant);
    let i = input(1, &[&g1], vec![anchor(&gid, 5, 1)]);
    let mut extra = notices(vec![], vec![]);
    if let Value::Obj(ms) = &mut extra {
        ms.push(("note".to_string(), s("x")));
    }
    let missing = obj(vec![("basis", nbasis()), ("revocations", Value::Arr(vec![]))]);
    let mut backwards = nbasis();
    if let Value::Obj(ms) = &mut backwards {
        for (k, v) in ms.iter_mut() {
            if k == "fromBlock" {
                *v = Value::Int(101);
            }
        }
    }
    let backwards = obj(vec![("basis", backwards), ("revocations", Value::Arr(vec![])), ("accessions", Value::Arr(vec![]))]);
    let mut other_chain = notice(&gid, 6, 0, 1, "counted");
    if let Value::Obj(ms) = &mut other_chain {
        ms[0].1 = Value::Int(CHAIN + 1);
    }
    let cases = [
        extra,
        missing,
        backwards,
        notices(vec![other_chain], vec![]),
        notices(vec![notice(&gid, 101, 0, 1, "counted")], vec![]),
        notices(vec![notice(&gid, 6, 0, 1, "counted"), notice(&format!("0x{}", "ee".repeat(32)), 6, 0, 2, "counted")], vec![]),
        notices(vec![notice(&gid, 6, 0, 1, "COUNTED")], vec![]),
        notices(vec![], vec![accession(6, 0, 1, 2, 1), accession(6, 0, 1, 3, 1)]),
        Value::Null,
        Value::Arr(vec![]),
    ];
    for (k, n) in cases.iter().enumerate() {
        assert!(read::load_notices(n).is_none(), "case {k} is no notice input");
        let r = check(&grant, Some(&i), Some(n));
        assert_eq!(r.member("notices"), Some(&Value::Null), "case {k}");
        assert_eq!(states(&r)[5], "UNKNOWN", "case {k}");
    }
}

#[test]
fn a_member_n_does_not_apply_to_reads_unknown() {
    let bad: [Value; 5] = [
        s("chain 7"),
        obj(vec![("chain", Value::Int(CHAIN))]),
        obj(vec![("chain", Value::Int(CHAIN)), ("at", s(&contract())), ("x", Value::Int(1))]),
        obj(vec![("chain", Value::Int(CHAIN + 1)), ("at", s(&contract()))]),
        obj(vec![("chain", Value::Int(CHAIN)), ("at", s(&format!("0x{}", "c1".repeat(20))))]),
    ];
    for (k, m) in bad.into_iter().enumerate() {
        let (g1, grant) = ledger(1, Some(m));
        let i = input(1, &[&g1], vec![anchor(&id(&grant), 5, 1)]);
        let r = check(&grant, Some(&i), Some(&notices(vec![], vec![])));
        let st = states(&r);
        assert_eq!((st[2].as_str(), st[3].as_str(), st[5].as_str()), ("UNKNOWN", "UNKNOWN", "UNKNOWN"), "case {k}");
        assert_eq!(verdict(&r), "PARTIAL", "case {k}");
    }
}

#[test]
fn the_book_says_who_is_in_office() {
    let (g1, grant) = ledger(1, Some(member()));
    let gid = id(&grant);
    let i = input(1, &[&g1], vec![anchor(&gid, 5, 1)]);
    let moved = accession(10, 0, 1, 2, 1);
    let retired = check(&grant, Some(&i), Some(&notices(vec![notice(&gid, 20, 0, 1, "counted")], vec![moved.clone()])));
    assert_eq!(states(&retired)[5], "PASS", "a retired key's notice withdraws nothing");
    let successor = check(&grant, Some(&i), Some(&notices(vec![notice(&gid, 20, 0, 2, "counted")], vec![moved.clone()])));
    assert_eq!(states(&successor)[5], "FAIL", "the key in office withdraws");
    let early = check(&grant, Some(&i), Some(&notices(vec![notice(&gid, 9, 0, 2, "counted")], vec![moved])));
    assert_eq!(states(&early)[5], "PASS", "before its accession the new key is not in office");
    let same_block = check(&grant, Some(&i), Some(&notices(vec![], vec![accession(5, 0, 1, 2, 1)])));
    assert_eq!(states(&same_block)[2], "PASS", "{:?}", states(&same_block));
}

#[test]
fn an_undecided_notice_is_no_acquittal_and_a_ledger_entry_no_withdrawal() {
    let (g1, grant) = ledger(1, Some(member()));
    let gid = id(&grant);
    let revocation = entry("zikaron/2", 1, "revocation", 2, Some(&id(&grant)), obj(vec![("grant", s(&gid))]));
    let i = input(1, &[&g1, &revocation], vec![anchor(&gid, 5, 1), anchor(&id(&revocation), 6, 1)]);
    for v in ["UNPROVEN", "VOID"] {
        let r = check(&grant, Some(&i), Some(&notices(vec![notice(&gid, 6, 0, 1, v)], vec![])));
        assert_eq!(states(&r)[5], "UNKNOWN", "{v}");
    }
    let entry_only = check(&grant, Some(&i), Some(&notices(vec![], vec![])));
    assert_eq!(states(&entry_only)[5], "PASS", "the entry is data; the notice is the act");
}

#[test]
fn a_spine_is_needed_and_a_forger_is_convicted() {
    let (g1, grant) = ledger(1, Some(member()));
    let gid = id(&grant);
    let spine_less = check(&grant, Some(&input(1, &[], vec![anchor(&gid, 5, 1)])), Some(&notices(vec![], vec![])));
    let st = states(&spine_less);
    assert_eq!((st[1].as_str(), st[2].as_str(), st[3].as_str(), st[5].as_str()), ("UNKNOWN", "PASS", "PASS", "PASS"), "a badge alone");
    let forged = entry("zikaron/2", 3, "grant", 1, Some(&id(&g1)), grant_body(Some(member())));
    let f = check(&forged, Some(&input(1, &[&g1], vec![anchor(&id(&forged), 5, 1)])), Some(&notices(vec![], vec![])));
    assert_eq!(states(&f)[2], "FAIL", "{:?}", states(&f));
    let (_, by_two) = ledger(2, Some(member()));
    let anchored_by_root = check(&by_two, Some(&input(1, &[&g1], vec![anchor(&id(&by_two), 5, 1)])), Some(&notices(vec![], vec![accession(1, 0, 1, 2, 1)])));
    let st = states(&anchored_by_root);
    assert_eq!(st[2], "UNKNOWN", "the standing record is the in-office key's; none by it: {st:?}");
}

#[test]
fn the_link_is_read_by_the_root_derived() {
    let up_g = genesis(1);
    let up = entry("zikaron/2", 1, "grant", 1, Some(&id(&up_g)), grant_body(None));
    let dg = genesis(2);
    let mut body = grant_body(Some(member()));
    if let Value::Obj(ms) = &mut body {
        ms.push(("upstream".to_string(), s(&id(&up))));
    }
    let down = entry("zikaron/2", 2, "grant", 1, Some(&id(&dg)), body);
    let seated = notices(vec![], vec![accession(1, 0, 9, 2, 9)]);
    let hops: Vec<read::Hop> = vec![(up.clone(), None, None), (down.clone(), None, Some(loaded(&seated)))];
    let r = read::chain_check(&hops, None);
    assert_eq!(r.member("links"), Some(&Value::Arr(vec![Value::Bool(true)])), "{r:?}");
    let unseated: Vec<read::Hop> = vec![(up, None, None), (down, None, Some(loaded(&notices(vec![], vec![]))))];
    let r = read::chain_check(&unseated, None);
    assert_eq!(r.member("links"), Some(&Value::Arr(vec![Value::Bool(false)])), "{r:?}");
}

fn zkk2(args: &[&str]) -> (i32, Vec<u8>) {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_zkk2")).args(args).output().expect("zkk2 runs");
    (out.status.code().unwrap_or(-1), out.stdout)
}

fn scratch(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("zkk2-notices-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch");
    d
}

#[test]
fn an_unreadable_notices_path_is_misuse_and_an_invalid_one_an_answer() {
    let d = scratch("paths");
    let (g1, grant) = ledger(1, Some(member()));
    let g = d.join("g.entry");
    std::fs::write(&g, &grant).expect("grant");
    let i = d.join("i.json");
    std::fs::write(&i, json::canon_bytes(&input(1, &[&g1], vec![anchor(&id(&grant), 5, 1)]))).expect("input");
    let mut unread = vec![("directory", d.display().to_string()), ("absent", d.join("none").display().to_string())];
    if cfg!(unix) {
        let pipe = d.join("pipe");
        assert!(std::process::Command::new("mkfifo").arg(&pipe).status().expect("mkfifo").success());
        unread.push(("pipe", pipe.display().to_string()));
    }
    let (g, i) = (g.display().to_string(), i.display().to_string());
    for (name, p) in unread {
        let (code, out) = zkk2(&["grant-check", &g, "--audit", &i, "--notices", &p]);
        assert_eq!((code, out.len()), (2, 0), "{name}");
    }
    let not_json = d.join("n.json");
    std::fs::write(&not_json, b"no json").expect("file");
    let (code, out) = zkk2(&["grant-check", &g, "--audit", &i, "--notices", &not_json.display().to_string()]);
    assert_eq!(code, 0);
    let r = json::parse(&out).expect("an answer");
    assert_eq!(r.member("notices"), Some(&Value::Null));
    if cfg!(unix) {
        let (code, out) = zkk2(&["grant-check", &g, "--audit", &i, "--notices", "/dev/null"]);
        assert_eq!((code, json::parse(&out).ok().and_then(|r| r.member("notices").cloned())), (0, Some(Value::Null)), "a character device is read");
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_hop_has_exactly_three_members() {
    let d = scratch("hops");
    let (g1, grant) = ledger(1, Some(member()));
    let gid = id(&grant);
    std::fs::write(d.join("g.entry"), &grant).expect("grant");
    std::fs::write(d.join("i.json"), json::canon_bytes(&input(1, &[&g1], vec![anchor(&gid, 5, 1)]))).expect("input");
    std::fs::write(d.join("n.json"), json::canon_bytes(&notices(vec![notice(&gid, 6, 0, 1, "counted")], vec![]))).expect("notices");
    let at = |n: &str| d.join(n).display().to_string();
    let hops = |members: &[(&str, &str)]| json::canon_bytes(&Value::Arr(vec![Value::Obj(members.iter().map(|(k, f)| (k.to_string(), s(&at(f)))).collect())]));
    std::fs::write(d.join("two.json"), hops(&[("grant", "g.entry"), ("audit", "i.json")])).expect("hops");
    assert_eq!(zkk2(&["chain-check", &at("two.json")]), (2, Vec::new()));
    std::fs::write(d.join("three.json"), hops(&[("grant", "g.entry"), ("audit", "i.json"), ("notices", "n.json")])).expect("hops");
    let (code, out) = zkk2(&["chain-check", &at("three.json")]);
    assert_eq!(code, 0);
    let r = json::parse(&out).expect("an answer");
    assert_eq!(r.member("verdict"), Some(&s("FAIL")));
    let hop = r.member("hops").and_then(Value::as_arr).and_then(|h| h.first()).cloned().unwrap_or(Value::Null);
    assert_eq!(canon(hop.member("notices")), canon(Some(&nbasis())));
    let _ = std::fs::remove_dir_all(&d);
}
