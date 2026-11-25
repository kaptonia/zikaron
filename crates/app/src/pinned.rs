pub use zikaron_anchor::deploy::CODE_HASH;

pub use zikaron_anchor::revoke::CODE_HASH as REVOCATION_CODE_HASH;

pub const CORE_DIGEST: &str = "0xbecfb6f0d0f8b71c314f1b2efef414abfb6df74b711ca8f81efdef685d0132fc";

pub const CORE_DIGEST_V2: &str = "0x2a6564e126c632c70b51c602b92bdee47ab2a86118b955d32e2dd5c0f1af567a";

pub const KIT_DIGEST_V2: &str = "0x5905333068c04ec2c4b096269f8baa0f2864925dc31910ae4e528e0c0899125d";

pub const FACE_DIGEST: &str = "0xc82c356c829fd0f88b543fc15a49deeb8088bc57635523372822e18b8ae53e8d";

#[cfg(test)]
mod tests {
    #[test]
    fn pinned_core_digests_equal_the_base_lock() {
        let lock = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../BASE-LOCK.json")).expect("BASE-LOCK.json");
        for (key, pinned) in [
            ("embeds", super::CORE_DIGEST),
            ("named", super::CORE_DIGEST_V2),
            ("named", super::KIT_DIGEST_V2),
        ] {
            let needle = format!("\"{key}\": \"{pinned}\"");
            assert!(lock.contains(&needle), "BASE-LOCK.json does not pin {key} = {pinned}");
        }
    }

    #[test]
    fn pinned_face_digest_equals_the_base_lock() {
        let lock = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../BASE-LOCK.json")).expect("BASE-LOCK.json");
        let lock = zikaron::json::parse(&lock).expect("the lock reads");
        let face = lock.member("cores").and_then(|c| c.member("zikaron.face/1")).expect("the lock names the face core");
        for (field, pinned) in [("named", super::FACE_DIGEST), ("computed", super::FACE_DIGEST), ("embeds", super::CORE_DIGEST_V2)] {
            assert_eq!(face.member(field).and_then(|x| x.as_str()), Some(pinned), "BASE-LOCK.json cores.zikaron.face/1.{field}");
        }
    }

    #[test]
    fn pinned_code_hashes_equal_the_base_lock() {
        let lock = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../BASE-LOCK.json")).expect("BASE-LOCK.json");
        let lock = zikaron::json::parse(&lock).expect("the lock reads");
        for (contract, pinned) in [("ZikaronRegistry", super::CODE_HASH), ("ZikaronRevocation", super::REVOCATION_CODE_HASH)] {
            let locked = lock.member("contracts").and_then(|c| c.member(contract)).and_then(|c| c.member("codeHash")).and_then(|h| h.as_str());
            assert_eq!(locked, Some(pinned), "BASE-LOCK.json pins {contract}");
        }
    }
}
