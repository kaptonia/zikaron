use zikaron::cryptox;
use zikaron::entry as v1;
use zikaron::hexfmt;
use zikaron::json::{self, Value};
use zikaron::tokens::Domain as V1Domain;
use zikaron_kit::kitdir::KitVerdict as KitVerdict1;
use zikaron_kit::v2::dispatch::{self, KitReading, Law, OldDocument};
use zikaron_kit::v2::doc::{self, Accepted};
use zikaron_kit::v2::kit::{self, Enumeration, KitVerdict};
use zikaron_kit::v2::read;
use zikaron_kit::v2::tokens::Rule;

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
    let domain = if spec == zikaron::tokens::SPEC { V1Domain::Entry.as_str() } else { zikaron::v2::tokens::Domain::Entry.as_str() };
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

fn h32(byte: &str) -> String {
    format!("0x{}", byte.repeat(32))
}

fn grant_body(grantee: u8) -> Value {
    obj(vec![("grantee", s(&addr(grantee))), ("work", s(&h32("aa"))), ("terms", s(&h32("bb")))])
}

fn anchor(hash: &str, block: u64, ts: u64, sender: u8) -> Value {
    obj(vec![
        ("chainId", Value::Int(1)),
        ("blockNumber", Value::Int(block)),
        ("blockTimestamp", Value::Int(ts)),
        ("tx", s(&format!("0x{:064x}", block))),
        ("sender", s(&addr(sender))),
        ("hash", s(hash)),
        ("verdict", s("counted")),
    ])
}

fn input(root: u8, pile: &[&Vec<u8>], anchors: Vec<Value>) -> Value {
    obj(vec![
        ("root", s(&addr(root))),
        ("pile", Value::Arr(pile.iter().map(|b| s(&hexfmt::encode(b))).collect())),
        ("anchors", Value::Arr(anchors)),
        (
            "basis",
            obj(vec![(
                "chains",
                Value::Arr(vec![obj(vec![
                    ("chainId", Value::Int(1)),
                    ("fromBlock", Value::Int(0)),
                    ("toBlock", Value::Int(1000)),
                    ("registries", Value::Arr(vec![s(&format!("0x{}", "11".repeat(20)))])),
                    ("senders", Value::Arr(vec![s(&addr(root))])),
                ])]),
            )]),
        ),
    ])
}

fn verdict(r: &Value) -> &str {
    r.member("verdict").and_then(Value::as_str).unwrap_or("")
}

fn states(r: &Value) -> Vec<String> {
    r.member("checks").and_then(Value::as_arr).into_iter().flatten().filter_map(|c| c.member("state").and_then(Value::as_str).map(str::to_string)).collect()
}

fn manifest(spec: &str, extra: Vec<(&str, Value)>) -> Vec<u8> {
    let mut ms = vec![
        ("spec", s(spec)),
        ("root", Value::Null),
        ("entries", Value::Arr(vec![])),
        ("files", Value::Arr(vec![])),
        ("contents", Value::Arr(vec![])),
        ("note_md", s("")),
    ];
    ms.extend(extra);
    json::canon_bytes(&obj(ms))
}

fn enumeration(pairs: Vec<(&str, Vec<u8>)>) -> Enumeration {
    pairs.into_iter().map(|(p, b)| (p.to_string(), b)).collect()
}

#[test]
fn an_old_grant_checks_green_after_its_issuer_upgrades() {
    let g1 = entry("zikaron/1", 1, "genesis", 0, None, obj(vec![("statement_md", s(""))]));
    let grant = entry("zikaron/1", 1, "grant", 1, Some(&id(&g1)), grant_body(7));
    let close = entry(
        "zikaron/1",
        1,
        "succession",
        2,
        Some(&id(&grant)),
        obj(vec![("to", s(&addr(1))), ("kind", s("migration")), ("effective", Value::Int(0)), ("statement_md", s(""))]),
    );
    let genesis = entry(
        "zikaron/2",
        1,
        "genesis",
        0,
        None,
        obj(vec![("statement_md", s("")), ("continues", obj(vec![("root", s(&addr(1))), ("entry_id", s(&id(&close)))]))]),
    );
    let anchors = vec![
        anchor(&id(&g1), 1, 100, 1),
        anchor(&id(&grant), 2, 110, 1),
        anchor(&id(&close), 3, 120, 1),
        anchor(&id(&genesis), 4, 130, 1),
    ];
    let inp = input(1, &[&g1, &close, &genesis], anchors);
    let view = read::load_view(&inp, Some(&grant)).expect("valid input");
    assert!(view.continued);
    let r = read::grant_check(&grant, Some(&view), None, None).result;
    assert_eq!(verdict(&r), "GREEN", "{:?}", states(&r));
}

#[test]
fn an_old_grant_without_a_continuation_is_partial() {
    let g1 = entry("zikaron/1", 1, "genesis", 0, None, obj(vec![("statement_md", s(""))]));
    let grant = entry("zikaron/1", 1, "grant", 1, Some(&id(&g1)), grant_body(7));
    let view = read::load_view(&input(1, &[&g1], vec![anchor(&id(&grant), 1, 100, 1)]), Some(&grant)).expect("valid");
    let r = read::grant_check(&grant, Some(&view), None, None).result;
    assert_eq!(verdict(&r), "PARTIAL");
    assert_eq!(states(&r)[2], "UNKNOWN");
}

#[test]
fn a_grant_package_needs_a_root() {
    let g = entry("zikaron/2", 1, "grant", 1, Some(&h32("00")), grant_body(7));
    let code = doc::badge_encode(std::slice::from_ref(&g)).expect("encode").into_bytes();
    let files = Value::Arr(vec![obj(vec![
        ("path", s("zikaron-grant.txt")),
        ("sha256", s(&id(&code))),
        ("size", Value::Int(code.len() as u64)),
    ])]);
    let pkg = |root: Value| {
        let m = json::canon_bytes(&obj(vec![
            ("spec", s("zikaron.kit/2")),
            ("root", root),
            ("entries", Value::Arr(vec![s(&id(&g))])),
            ("files", files.clone()),
            ("contents", Value::Arr(vec![])),
            ("note_md", s("")),
        ]));
        enumeration(vec![
            ("manifest.json", m),
            (&kit::entry_path(&id(&g)), g.clone()),
            ("files/zikaron-grant.txt", code.clone()),
        ])
    };
    let no_root = kit::verify_grant_package(&pkg(Value::Null));
    assert_eq!(verdict(&no_root), "E_GP_ROOT");
    let ok = kit::verify_grant_package(&pkg(s(&addr(1))));
    assert_eq!(verdict(&ok), "GP_OK");
    assert_eq!(ok.member("root").and_then(Value::as_str), Some(addr(1).as_str()));
}

fn directory_kit(row: impl Fn(&str) -> Vec<Value>) -> Enumeration {
    let bytes = b"inside".to_vec();
    let file = kit::FileRow { path: "dir/a.txt".into(), sha256: id(&bytes), size: bytes.len() as u64, name_md: None };
    let digest = kit::directory_manifest_digest(std::slice::from_ref(&file), "dir");
    let files = Value::Arr(vec![obj(vec![("path", s(&file.path)), ("sha256", s(&file.sha256)), ("size", Value::Int(file.size))])]);
    let m = json::canon_bytes(&obj(vec![
        ("spec", s("zikaron.kit/2")),
        ("root", Value::Null),
        ("entries", Value::Arr(vec![])),
        ("files", files),
        ("contents", Value::Arr(row(&digest))),
        ("note_md", s("")),
    ]));
    enumeration(vec![("manifest.json", m), ("files/dir/a.txt", bytes)])
}

fn contents_row(content: &str, name_md: Option<Value>) -> Value {
    let mut ms = vec![("content", s(content)), ("path", s("dir"))];
    if let Some(n) = name_md {
        ms.push(("name_md", n));
    }
    obj(ms)
}

#[test]
fn a_directory_originals_row_carries_any_string_as_its_name() {
    for name in ["季度报告", "", ".", "..", "a/b"] {
        let en = directory_kit(|d| vec![contents_row(d, Some(s(name)))]);
        let KitVerdict::Ok { manifest, .. } = kit::verify_kit(&en) else { panic!("{name:?} must pass §4.3") };
        assert_eq!(manifest.contents[0].name_md.as_deref(), Some(name));
    }
    let bare = directory_kit(|d| vec![contents_row(d, None)]);
    let KitVerdict::Ok { manifest, .. } = kit::verify_kit(&bare) else { panic!("a row without a name passes") };
    assert_eq!(manifest.contents[0].name_md, None);
}

#[test]
fn a_contents_row_outside_its_form_fails_contents() {
    let shapes: Vec<(&str, Box<dyn Fn(&str) -> Vec<Value>>)> = vec![
        ("int", Box::new(|d| vec![contents_row(d, Some(Value::Int(1)))])),
        ("null", Box::new(|d| vec![contents_row(d, Some(Value::Null))])),
        ("foreign", Box::new(|d| vec![obj(vec![("content", s(d)), ("path", s("dir")), ("name_md", s("n")), ("x", s("y"))])])),
        ("foreign without a name", Box::new(|d| vec![obj(vec![("content", s(d)), ("path", s("dir")), ("x", s("y"))])])),
        ("two names", Box::new(|d| vec![contents_row(d, Some(s("a"))), contents_row(d, Some(s("b")))])),
        ("named and bare", Box::new(|d| vec![contents_row(d, None), contents_row(d, Some(s("a")))])),
        ("not an object", Box::new(|d| vec![s(d)])),
    ];
    for (what, row) in &shapes {
        let en = directory_kit(row);
        match kit::verify_kit(&en) {
            KitVerdict::Fail { token, subject } => {
                assert_eq!(token.as_str(), "E_KIT_MANIFEST", "{what}");
                assert_eq!(subject.as_deref(), Some("contents"), "{what}");
            }
            KitVerdict::Ok { .. } => panic!("{what} must fail §4.3"),
        }
    }
}

#[test]
fn a_grant_package_result_row_has_two_members() {
    let original = b"the work".to_vec();
    let g = entry(
        "zikaron/2",
        1,
        "grant",
        1,
        Some(&h32("00")),
        obj(vec![("grantee", s(&addr(7))), ("work", s(&id(&original))), ("terms", s(&h32("bb")))]),
    );
    let code = doc::badge_encode(std::slice::from_ref(&g)).expect("encode").into_bytes();
    let file_row = |path: &str, b: &[u8]| obj(vec![("path", s(path)), ("sha256", s(&id(b))), ("size", Value::Int(b.len() as u64))]);
    let m = json::canon_bytes(&obj(vec![
        ("spec", s("zikaron.kit/2")),
        ("root", s(&addr(1))),
        ("entries", Value::Arr(vec![s(&id(&g))])),
        ("files", Value::Arr(vec![file_row("original.bin", &original), file_row("zikaron-grant.txt", &code)])),
        ("contents", Value::Arr(vec![obj(vec![("content", s(&id(&original))), ("name_md", s("named")), ("path", s("original.bin"))])])),
        ("note_md", s("")),
    ]));
    let en = enumeration(vec![
        ("manifest.json", m),
        (&kit::entry_path(&id(&g)), g.clone()),
        ("files/original.bin", original.clone()),
        ("files/zikaron-grant.txt", code.clone()),
    ]);
    let r = kit::verify_grant_package(&en);
    assert_eq!(verdict(&r), "GP_OK", "{}", String::from_utf8_lossy(&json::canon_bytes(&r)));
    let want = obj(vec![("content", s(&id(&original))), ("path", s("original.bin"))]);
    assert_eq!(r.member("contents"), Some(&Value::Arr(vec![want])));
}

#[test]
fn a_kit1_kit_is_read_by_kit1_and_refused_by_kit2() {
    let m1 = manifest("zikaron.kit/1", vec![("proofs", Value::Arr(vec![]))]);
    assert_eq!(kit::check_manifest(&m1).err(), Some(Rule::Members));
    let en = enumeration(vec![("manifest.json", m1.clone())]);
    assert_eq!(dispatch::manifest_law(Some(&m1)), Law::Kit1);
    assert!(matches!(dispatch::read_kit(&en), KitReading::Kit1(KitVerdict1::Ok { .. })));
    let m2 = manifest("zikaron.kit/2", vec![]);
    let en2 = enumeration(vec![("manifest.json", m2)]);
    assert!(matches!(dispatch::read_kit(&en2), KitReading::Kit2(KitVerdict::Ok { .. })));
}

#[test]
fn a_one_file_kit_is_read_under_the_law_its_manifest_names() {
    let one_file = |m: &[u8]| {
        let mut b = b"zikaron-kit-file/1\nmanifest.json\n".to_vec();
        b.extend_from_slice(m.len().to_string().as_bytes());
        b.push(b'\n');
        b.extend_from_slice(m);
        b.push(b'\n');
        b
    };
    let m1 = manifest("zikaron.kit/1", vec![("proofs", Value::Arr(vec![]))]);
    assert!(matches!(dispatch::read_kit_file(&one_file(&m1)), KitReading::Kit1(KitVerdict1::Ok { .. })));
    let m2 = manifest("zikaron.kit/2", vec![]);
    assert!(matches!(dispatch::read_kit_file(&one_file(&m2)), KitReading::Kit2(KitVerdict::Ok { .. })));
    assert!(matches!(
        dispatch::read_kit_file(b"not a kit"),
        KitReading::Kit2(KitVerdict::Fail { subject: Some(ref x), .. }) if x == "."
    ));
}

#[test]
fn old_documents_go_to_kit1() {
    let fpm = json::canon_bytes(&obj(vec![("spec", s("zikaron.fpm/1"))]));
    assert!(matches!(dispatch::read_old_document(&fpm), Some(OldDocument::Fpm(Err(_)))));
    let ack = json::canon_bytes(&obj(vec![("spec", s("zikaron.ack/1"))]));
    assert!(matches!(dispatch::read_old_document(&ack), Some(OldDocument::Ack(Err(_)))));
    assert!(dispatch::read_old_document(&manifest("zikaron.kit/2", vec![])).is_none());
}

#[test]
fn one_file_order_and_subjects() {
    let item = |p: &str, b: &[u8]| {
        let mut v = format!("{p}\n{}\n", b.len()).into_bytes();
        v.extend_from_slice(b);
        v.push(b'\n');
        v
    };
    let mut ok = b"zikaron-kit-file/1\n".to_vec();
    ok.extend(item("manifest.json", b"{}"));
    ok.extend(item("a", b"1"));
    ok.extend(item("b", b"2"));
    assert_eq!(kit::parse_one_file(&ok).map(|e| e.len()), Ok(3));
    let mut back = b"zikaron-kit-file/1\n".to_vec();
    back.extend(item("manifest.json", b"{}"));
    back.extend(item("b", b"2"));
    back.extend(item("a", b"1"));
    assert_eq!(kit::parse_one_file(&back), Err("a".to_string()));
    assert_eq!(kit::parse_one_file(b"zikaron-kit-file/1\n"), Err(".".to_string()));
}

#[test]
fn an_old_badge_still_decodes() {
    let g = entry("zikaron/1", 1, "grant", 1, Some(&h32("00")), grant_body(7));
    assert!(matches!(doc::accept(&g), Ok(Accepted::V1(_))));
    let p = doc::badge_encode(std::slice::from_ref(&g)).expect("encode");
    assert_eq!(doc::badge_decode(p.as_bytes()).map(|v| v.len()).ok(), Some(1));
}
