pub const MAGIC: &[u8] = b"zikaron-local/1\n";

pub const MAGIC_V2: &[u8] = b"zikaron-local/2\n";

pub fn is_sealed(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC_V2)
}
