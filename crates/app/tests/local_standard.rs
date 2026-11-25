use app::fault::Known;
use app::local::{envelope, opens_under_key, unseal, Doc, Expect, Ident, Owner, Unread, ALG_KEYED, DIGEST, MAGIC, MAGIC_V2, PLACE_DOMAIN};

fn key() -> [u8; 32] {
    std::array::from_fn(|i| i as u8)
}

fn nonce() -> [u8; 24] {
    std::array::from_fn(|i| 0x40 + i as u8)
}

const HOME: &str = "000102030405060708090a0b0c0d0e0f";

struct Vector {
    owner: Owner,
    doc: Doc,
    logical: &'static str,
    plain: &'static [u8],
    digest: &'static str,
    sealed: &'static str,
}

fn vectors() -> Vec<Vector> {
    vec![
        Vector {
            owner: Owner::Home(HOME.into()),
            doc: Doc::Settings,
            logical: "settings/settings.json",
            plain: b"{\"capBytes\":0}",
            digest: "20b1f7730575bd1d9c6e7f6657d4a7b8",
            sealed: "7a696b61726f6e2d6c6f63616c2f320a020873657474696e6773000120b1f7730575bd1d9c6e7f6657d4a7b8404142434445464748494a4b4c4d4e4f5051525354555657d41c6d1fbd854326bfc4b68e9dac56a2a68a98f4256964aa5201c475683441a075c9665e30a2e210e9c191387eb3b496e8440e8d1bf1ddf2d18f269e60dd222f7848990256ca31adf6218acfa12a52118d8dd0437412eab0a6179317fb",
        },
        Vector {
            owner: Owner::Machine,
            doc: Doc::Registry,
            logical: "identities-anchor.json",
            plain: b"{\"rows\":[]}",
            digest: "fd4681f3485cef8efd548179e988c78d",
            sealed: "7a696b61726f6e2d6c6f63616c2f320a020872656769737472790001fd4681f3485cef8efd548179e988c78d404142434445464748494a4b4c4d4e4f5051525354555657d43e6811b3881078eaf491d7cbf90be6fbcec4a1607432f409599237276e50ff7882201c3ae5f732c5e9a9313d51cb8dbfc54d5c366c3d2352dcce95",
        },
        Vector {
            owner: Owner::Label,
            doc: Doc::HomeLabel,
            logical: "settings/home-label.json",
            plain: b"{\"home\":\"000102030405060708090a0b0c0d0e0f\",\"owner\":\"none\"}",
            digest: "59b8b374c2f7a7527e6ffd99f8ce6dc2",
            sealed: "7a696b61726f6e2d6c6f63616c2f320a020a686f6d652d6c6162656c000159b8b374c2f7a7527e6ffd99f8ce6dc2404142434445464748494a4b4c4d4e4f5051525354555657d4336d1fbd85547aee96e2d2af8416f7e6cec4aa742a7cf2055c9868656541f57ad7681d3afcff3297dd992928e0f8c1ab5b4cd85db587ac82cc3dc425827b64621bc142758327f8e633d4cfb949facddae5695c465816556a8169fbab0011584b0e9800fcd86d2ff676a066c3449883",
        },
    ]
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn ident(v: &Vector) -> Ident {
    Ident { owner: v.owner.clone(), doc: v.doc, logical: v.logical.to_string() }
}

fn expect(v: &Vector) -> Expect {
    Expect { owner: Some(v.owner.clone()), doc: v.doc, rel: Some(v.logical.to_string()) }
}

fn reason(r: Result<Vec<u8>, app::fault::Fault>) -> Option<Unread> {
    match r {
        Ok(_) => None,
        Err(f) => {
            assert_eq!(f.which(), Some(Known::LocalSeal), "one member for every reason: {}", f.said());
            Unread::of(&f)
        }
    }
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex")).collect()
}

#[test]
fn the_test_vectors_seal_byte_for_byte_and_open_back() {
    for v in vectors() {
        let id = ident(&v);
        let sealed = envelope(&key(), &id, &nonce(), v.plain).expect("seals");
        assert_eq!(hex(&id.digest_keyed(&key())), v.digest, "{} digest", v.doc.tag());
        assert_eq!(hex(&sealed), v.sealed, "{} envelope", v.doc.tag());
        assert_eq!(unseal(&key(), &expect(&v), &sealed, "vector").expect("opens"), v.plain);
        assert_eq!(unseal(&key(), &expect(&v), &unhex(v.sealed), "vector").expect("the vector opens"), v.plain);
    }
}

fn guess(logical: &str) -> [u8; DIGEST / 2] {
    let mut msg = PLACE_DOMAIN.as_bytes().to_vec();
    msg.push(0);
    msg.extend_from_slice(logical.as_bytes());
    let d = zikaron::cryptox::sha256(&msg);
    std::array::from_fn(|i| d[i])
}

#[test]
fn a_place_half_is_keyed_and_a_guess_by_name_does_not_match() {
    for v in vectors() {
        let id = ident(&v);
        let sealed = envelope(&key(), &id, &nonce(), v.plain).unwrap();
        let d_at = MAGIC_V2.len() + 4 + v.doc.tag().len();
        let half = &sealed[d_at + DIGEST / 2..d_at + DIGEST];
        assert_ne!(half, &guess(v.logical)[..], "{}: the name's unkeyed digest is not the half", v.doc.tag());
        assert_ne!(half, &id.digest_keyed(&[0xffu8; 32])[DIGEST / 2..], "{}: under another key, another half", v.doc.tag());
        assert_eq!(&sealed[d_at..d_at + DIGEST / 2], &id.digest_keyed(&[0xffu8; 32])[..DIGEST / 2], "{}: the owner's half under any key", v.doc.tag());
    }
}

#[test]
fn the_head_carries_magic_algorithm_kind_version_digest_and_nonce() {
    let v = &vectors()[0];
    let id = ident(v);
    let s = envelope(&key(), &id, &nonce(), v.plain).unwrap();
    let mut at = MAGIC_V2.len();
    assert_eq!(&s[..at], MAGIC_V2);
    assert_eq!(s[at], ALG_KEYED);
    at += 1;
    let tag = v.doc.tag().as_bytes();
    assert_eq!(s[at] as usize, tag.len());
    at += 1;
    assert_eq!(&s[at..at + tag.len()], tag);
    at += tag.len();
    assert_eq!(&s[at..at + 2], &v.doc.version().to_be_bytes());
    at += 2;
    assert_eq!(&s[at..at + DIGEST], &id.digest_keyed(&key()));
    at += DIGEST;
    assert_eq!(&s[at..at + 24], &nonce());
    let other_place = Ident { logical: "settings/queue.json".into(), doc: Doc::Queue, ..id.clone() };
    let other_owner = Ident { owner: Owner::Machine, ..id.clone() };
    let k = key();
    assert_eq!(id.digest_keyed(&k)[..8], other_place.digest_keyed(&k)[..8]);
    assert_ne!(id.digest_keyed(&k)[8..], other_place.digest_keyed(&k)[8..]);
    assert_eq!(id.digest_keyed(&k)[8..], other_owner.digest_keyed(&k)[8..]);
    assert_ne!(id.digest_keyed(&k)[..8], other_owner.digest_keyed(&k)[..8]);
}

#[test]
fn bytes_without_the_magic_are_not_sealed() {
    let v = &vectors()[0];
    for b in [&b""[..], v.plain, b"zikaron-local/3\n", b"ZIKARON-LOCAL/2\n"] {
        assert_eq!(reason(unseal(&key(), &expect(v), b, "t")), Some(Unread::NotSealed), "{b:?}");
    }
}

#[test]
fn a_head_cut_anywhere_is_cut_short() {
    let v = &vectors()[0];
    let s = envelope(&key(), &ident(v), &nonce(), v.plain).unwrap();
    let head = MAGIC_V2.len() + 1 + 1 + v.doc.tag().len() + 2 + DIGEST + 24;
    for n in MAGIC_V2.len()..head + 16 {
        assert_eq!(reason(unseal(&key(), &expect(v), &s[..n], "t")), Some(Unread::Truncated), "cut at {n}");
    }
}

#[test]
fn another_kind_is_another_kind() {
    let v = &vectors()[0];
    let s = envelope(&key(), &ident(v), &nonce(), v.plain).unwrap();
    let as_queue = Expect { doc: Doc::Queue, ..expect(v) };
    assert_eq!(reason(unseal(&key(), &as_queue, &s, "t")), Some(Unread::OtherKind));
}

#[test]
fn a_newer_version_or_algorithm_is_newer() {
    let v = &vectors()[0];
    let s = envelope(&key(), &ident(v), &nonce(), v.plain).unwrap();
    let ver_at = MAGIC_V2.len() + 2 + v.doc.tag().len();
    let mut newer = s.clone();
    newer[ver_at..ver_at + 2].copy_from_slice(&2u16.to_be_bytes());
    assert_eq!(reason(unseal(&key(), &expect(v), &newer, "t")), Some(Unread::NewerVersion));
    let mut alg = s.clone();
    alg[MAGIC_V2.len()] = ALG_KEYED + 1;
    assert_eq!(reason(unseal(&key(), &expect(v), &alg, "t")), Some(Unread::NewerVersion));
    for older in 0..ALG_KEYED {
        let mut alg = s.clone();
        alg[MAGIC_V2.len()] = older;
        assert_eq!(reason(unseal(&key(), &expect(v), &alg, "t")), Some(Unread::NotSealed), "algorithm {older}");
    }
    let mut zero = s.clone();
    zero[ver_at..ver_at + 2].copy_from_slice(&0u16.to_be_bytes());
    assert_eq!(reason(unseal(&key(), &expect(v), &zero, "t")), Some(Unread::Unopenable));
}

#[test]
fn another_owner_or_place_is_another_file_before_any_key() {
    let v = &vectors()[0];
    let s = envelope(&key(), &ident(v), &nonce(), v.plain).unwrap();
    let wrong_key = [0xffu8; 32];
    for ex in [
        Expect { owner: Some(Owner::Home("ffffffffffffffffffffffffffffffff".into())), ..expect(v) },
        Expect { owner: Some(Owner::Machine), ..expect(v) },
        Expect { owner: None, ..expect(v) },
    ] {
        assert_eq!(reason(unseal(&key(), &ex, &s, "t")), Some(Unread::Swapped), "{ex:?}");
        assert_eq!(reason(unseal(&wrong_key, &ex, &s, "t")), Some(Unread::Swapped), "before the key: {ex:?}");
    }
    let elsewhere = Expect { rel: Some("settings/other.json".into()), ..expect(v) };
    assert_eq!(reason(unseal(&key(), &elsewhere, &s, "t")), Some(Unread::Swapped), "another place, the right key");
    assert_eq!(reason(unseal(&wrong_key, &elsewhere, &s, "t")), Some(Unread::Unopenable), "another place, a wrong key");
    let other = Ident { owner: Owner::Home("ffffffffffffffffffffffffffffffff".into()), ..ident(v) };
    let mut forged = s.clone();
    let d_at = MAGIC_V2.len() + 4 + v.doc.tag().len();
    forged[d_at..d_at + DIGEST].copy_from_slice(&other.digest_keyed(&key()));
    let ex = Expect { owner: Some(other.owner.clone()), ..expect(v) };
    assert_eq!(reason(unseal(&key(), &ex, &forged, "t")), Some(Unread::Unopenable));
}

#[test]
fn a_wrong_key_or_any_changed_byte_does_not_open() {
    let v = &vectors()[0];
    let s = envelope(&key(), &ident(v), &nonce(), v.plain).unwrap();
    assert_eq!(reason(unseal(&[7u8; 32], &expect(v), &s, "t")), Some(Unread::Unopenable));
    let mut ct = s.clone();
    let last = ct.len() - 1;
    ct[last] ^= 1;
    assert_eq!(reason(unseal(&key(), &expect(v), &ct, "t")), Some(Unread::Unopenable));
    let mut n = s.clone();
    let n_at = MAGIC_V2.len() + 4 + v.doc.tag().len() + DIGEST;
    n[n_at] ^= 1;
    assert_eq!(reason(unseal(&key(), &expect(v), &n, "t")), Some(Unread::Unopenable));
    assert!(opens_under_key(&key(), &s) && !opens_under_key(&[7u8; 32], &s));
}

#[test]
fn the_first_envelope_is_not_sealed() {
    let v = &vectors()[0];
    let tag = v.doc.tag().as_bytes();
    let mut head = MAGIC.to_vec();
    head.push(tag.len() as u8);
    head.extend_from_slice(tag);
    head.extend_from_slice(&1u16.to_be_bytes());
    let ct = app::cryptx::xchacha_seal(&key(), &nonce(), &head, v.plain).unwrap();
    let mut old = head.clone();
    old.extend_from_slice(&nonce());
    old.extend_from_slice(&ct);
    let anyone = Expect { owner: None, ..expect(v) };
    for (k, ex) in [(key(), expect(v)), (key(), anyone), ([7u8; 32], expect(v))] {
        assert_eq!(reason(unseal(&k, &ex, &old, "t")), Some(Unread::NotSealed), "{ex:?}");
    }
    assert!(!opens_under_key(&key(), &old));
}

#[test]
fn the_six_reasons_read_back_from_their_refusal() {
    assert_eq!(Unread::ALL.len(), 6);
    for u in Unread::ALL {
        assert_eq!(Unread::of(&u.fault(Doc::Settings, "x")), Some(u));
    }
    assert_eq!(Unread::of(&app::fault::Fault::known(Known::Locked, String::new())), None);
}

