//! The pinned AppImage runtime for the Linux package: the `type2-runtime` build the shipped packages carry
//! (commit [`COMMIT`]), identified by its SHA-256 ([`SHA256`]). Any other bytes (the moving `continuous`
//! download, a truncated file, another build) are refused by name before packaging, so rebuilding an AppImage
//! from the same source yields the same runtime.

/// The `type2-runtime` commit the pinned runtime was built from (its `--appimage-version` names it).
pub const COMMIT: &str = "8f39b89";

/// The pinned runtime's SHA-256 (the runtime of the 0.1.0 and 0.1.1 AppImages).
pub const SHA256: &str = "156f4bdbde9c52d01814600013e0a273f0118dc2de98975f3c8c63427ec79074";

/// Architectures with a pinned runtime, each with its digest. Only x86_64: the shipped AppImages are x86_64 and
/// no other architecture's runtime has been recorded.
pub const PINNED: [(&str, &str); 1] = [("x86_64", SHA256)];

/// Whether `bytes` are the pinned runtime: `Ok` with its digest, or an error naming the found and expected
/// digests.
pub fn check(bytes: &[u8]) -> Result<String, String> {
    check_against(bytes, SHA256)
}

/// [`check`] for the runtime of `arch`: an architecture with no pinned runtime is refused by name.
pub fn check_for(arch: &str, bytes: &[u8]) -> Result<String, String> {
    match PINNED.iter().find(|(a, _)| *a == arch) {
        Some((_, want)) => check_against(bytes, want),
        None => Err(format!("no AppImage runtime is pinned for {arch} (pinned: {})", PINNED.iter().map(|(a, _)| *a).collect::<Vec<_>>().join(", "))),
    }
}

/// [`check`] against the digest given.
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

    /// The pinned digest is well-formed; matching bytes pass, any other bytes (one byte changed, truncated,
    /// empty) are refused naming both digests.
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

    /// The runtime is checked per architecture: x86_64 against its pin, an architecture with none refused by
    /// name before any bytes are compared.
    #[test]
    fn an_architecture_without_a_pin_is_refused_by_name() {
        let e = check_for("x86_64", b"other bytes").err().expect("not the pinned bytes");
        assert!(e.contains(SHA256), "{e}");
        let e = check_for("aarch64", b"anything").err().expect("no pin");
        assert!(e.contains("no AppImage runtime is pinned for aarch64") && e.contains("x86_64"), "{e}");
    }
}
