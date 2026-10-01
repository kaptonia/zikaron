//! The two keystore cryptography pieces at the `cryptx` boundary (scrypt and AES-128-CTR), against standard
//! test vectors.
//!
//! These vectors are numbers printed in SP 800-38A and RFC 7914, not expectations we computed ourselves. A
//! mismatch turns red the same day. The BIP-39 and BIP-32 vectors live in unit tests inside `cryptx`
//! (derivation does not leave that module).

use zikaron::hexfmt;

fn hex(s: &str) -> Vec<u8> {
    hexfmt::decode(&format!("0x{s}")).expect("测试向量的十六进制")
}

#[test]
fn aes128_ctr_matches_sp_800_38a_f_5_1() {
    // SP 800-38A appendix F.5.1: CTR-AES128.Encrypt, first block.
    let key: [u8; 16] = hex("2b7e151628aed2a6abf7158809cf4f3c").try_into().unwrap();
    let iv: [u8; 16] = hex("f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff").try_into().unwrap();
    let mut buf = hex("6bc1bee22e409f96e93d7e117393172a");
    app::cryptx::aes128_ctr(&key, &iv, &mut buf);
    assert_eq!(hexfmt::encode(&buf), "0x874d6191b620e3261bef6864990db6ce");
}

#[test]
fn aes_ctr_is_its_own_inverse() {
    let key = [7u8; 16];
    let iv = [3u8; 16];
    let clear = b"keystore V3 \xe7\x9a\x84\xe5\xaf\x86\xe6\x96\x87\xe8\xb5\xb0\xe8\xbf\x99\xe4\xb8\x80\xe6\x9d\xa1";
    let mut buf = clear.to_vec();
    app::cryptx::aes128_ctr(&key, &iv, &mut buf);
    assert_ne!(&buf, clear, "密文与明文一样即没有加密");
    app::cryptx::aes128_ctr(&key, &iv, &mut buf);
    assert_eq!(&buf, clear, "同一条路走回来即原文");
}

#[test]
fn scrypt_matches_rfc_7914_first_vector() {
    // RFC 7914 §11 first set: P and S both empty, N=16 r=1 p=1 dkLen=64.
    let mut out = [0u8; 64];
    assert!(app::cryptx::scrypt(b"", b"", 16, 1, 1, &mut out));
    assert_eq!(
        hexfmt::encode(&out),
        "0x77d6576238657b203b19ca42c18a0497f16b4844e3074ae8dfdffa3fede21442\
fcd0069ded0948f8326a753a0fc81f17e8d3e0fb2e0d3628cf35e20c38d18906"
    );
}

#[test]
fn scrypt_matches_rfc_7914_second_vector() {
    // RFC 7914 §11 second set: password / NaCl, N=1024 r=8 p=16.
    let mut out = [0u8; 64];
    assert!(app::cryptx::scrypt(b"password", b"NaCl", 1024, 8, 16, &mut out));
    assert_eq!(
        hexfmt::encode(&out),
        "0xfdbabe1c9d3472007856e7190d01e9fe7c6ad7cbc8237830e77376634b373162\
2eaf30d92e22a3886ff109279d9830dac727afb94a83ee6d8360cbdfa2cc0640"
    );
}

#[test]
fn scrypt_refuses_a_shape_it_cannot_honour() {
    let mut out = [0u8; 32];
    assert!(!app::cryptx::scrypt(b"x", b"y", 15, 1, 1, &mut out), "N 不是二的幂即拒");
    assert!(!app::cryptx::scrypt(b"x", b"y", 1, 1, 1, &mut out), "N 须大于一");
    assert!(!app::cryptx::scrypt(b"x", b"y", 16, 0, 1, &mut out), "r 为零即拒");
}

// ───────────────────────── keystore V3's three testable behaviors ─────────────────────────

/// The raw bytes of this test private key. Only here; anything that needs it takes this constant.
const RAW: [u8; 32] = [
    0x4c, 0x0b, 0x39, 0x8f, 0x21, 0x77, 0x0a, 0x5e, 0x11, 0x2c, 0x63, 0x84, 0x9d, 0x02, 0xb7, 0x1f,
    0x88, 0x45, 0xea, 0x30, 0x7c, 0x59, 0x16, 0xd3, 0x6a, 0xf1, 0x08, 0x4b, 0x92, 0x27, 0xc5, 0x60,
];

fn a_key() -> app::key::Secret {
    app::key::Secret::take(RAW).expect("这一枚在曲线阶内")
}

/// `Secret` has no `Debug` (it cannot be printed), so `expect_err` cannot be used: unwrap it by hand.
fn must_refuse(r: Result<app::key::Secret, app::fault::Fault>) -> app::fault::Fault {
    match r {
        Ok(_) => panic!("这一份该被拒,却解开了"),
        Err(e) => e,
    }
}

#[test]
fn keystore_round_trips_back_to_the_same_address() {
    let k = a_key();
    let want = k.address().expect("地址");
    let ks = app::keystore::encrypt(&k, "一个够长的密码", app::keystore::Params::light(), 1_700_000_000)
        .expect("加密");
    assert_eq!(ks.address, want);
    let back = app::keystore::decrypt(&ks.json, "一个够长的密码").expect("解密");
    assert_eq!(back.address().expect("地址"), want, "密码解出来该是同一个地址");
}

#[test]
fn a_wrong_password_is_refused_at_the_mac_and_named() {
    let ks = app::keystore::encrypt(&a_key(), "对的密码", app::keystore::Params::light(), 1_700_000_000)
        .expect("加密");
    let e = must_refuse(app::keystore::decrypt(&ks.json, "错的密码"));
    assert!(e.said().starts_with("BAD_PASSWORD"), "错密码要具名说出来,现读:{}", e.said());
    assert!(!e.tail().is_empty(), "证据尾照带");
}

#[test]
fn the_written_shape_is_a_compliant_v3() {
    let ks = app::keystore::encrypt(&a_key(), "密码", app::keystore::Params::light(), 1_700_000_000)
        .expect("加密");
    let s = app::keystore::shape(&ks.json).expect("读形");
    assert!(s.compliant().is_empty(), "不合的栏:{:?}", s.compliant());
    assert_eq!(s.version, 3);
    assert_eq!(s.kdf, "scrypt");
    assert_eq!(s.cipher, "aes-128-ctr");
    assert_eq!(s.dklen, 32);
    assert_eq!(s.iv_len, 16);
    assert_eq!(s.address, Some(ks.address));
}

#[test]
fn the_file_name_follows_the_utc_convention() {
    let ks = app::keystore::encrypt(&a_key(), "密码", app::keystore::Params::light(), 1_700_000_000)
        .expect("加密");
    // 1700000000 = 2023-11-14T22:13:20Z
    assert!(
        ks.file_name.starts_with("UTC--2023-11-14T22-13-20.000000000Z--"),
        "现读:{}",
        ks.file_name
    );
    assert!(ks.file_name.ends_with(&ks.address.hex()[2..]), "名字末尾是地址");
}

#[test]
fn not_one_byte_of_the_plaintext_key_appears_in_what_is_written() {
    let k = a_key();
    let ks = app::keystore::encrypt(&k, "密码", app::keystore::Params::light(), 1_700_000_000)
        .expect("加密");
    let _ = &k;
    // Scan for both spellings: the hex string (both cases) and the thirty-two raw bytes themselves.
    let hex = zikaron::hexfmt::encode(&RAW);
    let bare = &hex[2..];
    let text = String::from_utf8_lossy(&ks.json).to_string();
    assert!(!text.contains(bare), "落盘的那一份里有明文私钥(小写十六进制)");
    assert!(!text.contains(&bare.to_uppercase()), "落盘的那一份里有明文私钥(大写十六进制)");
    assert!(
        !ks.json.windows(RAW.len()).any(|w| w == RAW),
        "落盘的那一份里有明文私钥的裸字节"
    );
}

#[test]
fn a_keystore_whose_shape_is_off_is_refused_by_the_named_column() {
    let ks = app::keystore::encrypt(&a_key(), "密码", app::keystore::Params::light(), 1_700_000_000)
        .expect("加密");
    let broken = String::from_utf8_lossy(&ks.json).replace("\"version\":3", "\"version\":1");
    let e = must_refuse(app::keystore::decrypt(broken.as_bytes(), "密码"));
    assert!(e.said().starts_with("KEYSTORE_SHAPE"), "现读:{}", e.said());
    assert!(e.tail().contains("version"), "要说出是哪一栏,现读:{}", e.tail());
}
