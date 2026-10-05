//! The two values this binary pins from the base. They are written here, in the product's own sources, so the
//! product builds from its own tree alone and never reads the base; the test driver hands them beside the base
//! lock's own values (`BASE-LOCK.json` `codeHash`, `coreDigest`), and the comparison is the gauges'.

/// The registry build's runtime code hash (keccak-256 of the runtime bytecode), `0x` and sixty-four lowercase
/// hex digits: `base/zikaron-core/contracts/CODEHASH.md`.
pub const CODE_HASH: &str = "0xfa97a1d9b22fab2b52f4e27c9a965b32734c40001b565ab365d05c887118f57d";

/// The release digest of the `zikaron/1` core that judges every audit in this binary: `base/zikaron-v1.md`
/// §12.5.
pub const CORE_DIGEST: &str = "0xbecfb6f0d0f8b71c314f1b2efef414abfb6df74b711ca8f81efdef685d0132fc";
