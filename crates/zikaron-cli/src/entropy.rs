pub fn bytes32() -> Option<[u8; 32]> {
    crate::seam();
    let mut buf = [0u8; 32];
    zikaron_os::fill_random(&mut buf).ok()?;
    Some(buf)
}
