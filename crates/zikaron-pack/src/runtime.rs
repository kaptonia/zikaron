pub const COMMIT: &str = "8f39b89";

pub const SHA256: &str = "156f4bdbde9c52d01814600013e0a273f0118dc2de98975f3c8c63427ec79074";

pub const PINNED: [(&str, &str); 1] = [("x86_64", SHA256)];

pub fn check(bytes: &[u8]) -> Result<String, String> {
    check_against(bytes, SHA256)
}

pub fn check_for(arch: &str, bytes: &[u8]) -> Result<String, String> {
    match PINNED.iter().find(|(a, _)| *a == arch) {
        Some((_, want)) => check_against(bytes, want),
        None => Err(format!("no AppImage runtime is pinned for {arch} (pinned: {})", PINNED.iter().map(|(a, _)| *a).collect::<Vec<_>>().join(", "))),
    }
}

pub fn check_against(bytes: &[u8], want: &str) -> Result<String, String> {
    let got = crate::sha256::hex(bytes);
    if got == want {
        Ok(got)
    } else {
        Err(format!("the AppImage runtime is not the pinned one (type2-runtime {COMMIT}): sha256 {got} of {} bytes, wanted {want}", bytes.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_pinned_bytes_pass() {
        assert_eq!(SHA256.len(), 64);
        assert!(SHA256.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
        let bytes = b"a runtime".to_vec();
        let want = crate::sha256::hex(&bytes);
        assert_eq!(check_against(&bytes, &want), Ok(want.clone()));
        let mut other = bytes.clone();
        other[0] ^= 1;
        for (form, b) in [("one byte changed", other), ("one byte cut", bytes[..bytes.len() - 1].to_vec()), ("nothing", Vec::new())] {
            let e = check_against(&b, &want).err().unwrap_or_else(|| panic!("{form}: refused"));
            assert!(e.contains(&want) && e.contains(&crate::sha256::hex(&b)) && e.contains(COMMIT), "{form}: {e}");
        }
    }

    #[test]
    fn an_architecture_without_a_pin_is_refused_by_name() {
        let e = check_for("x86_64", b"other bytes").err().expect("not the pinned bytes");
        assert!(e.contains(SHA256), "{e}");
        let e = check_for("aarch64", b"anything").err().expect("no pin");
        assert!(e.contains("no AppImage runtime is pinned for aarch64") && e.contains("x86_64"), "{e}");
    }
}
