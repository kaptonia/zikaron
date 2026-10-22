//! Values this binary pins from the protocol base. They are copied into the app's own sources so the build
//! never reads `base/`; tests compare them with `BASE-LOCK.json` (`codeHash`, `coreDigest`).

/// The registry build's runtime code hash (keccak-256 of the runtime bytecode), `0x` and sixty-four lowercase
/// hex digits: `base/zikaron-core/contracts/CODEHASH.md`. Pinned once, where the deployments are
/// (`zikaron_anchor::deploy`), for the desktop and the command line alike.
pub use zikaron_anchor::deploy::CODE_HASH;

/// The release digest of the `zikaron/1` core that performs every audit in this binary: `base/zikaron-v1.md`
/// §12.5.
pub const CORE_DIGEST: &str = "0xbecfb6f0d0f8b71c314f1b2efef414abfb6df74b711ca8f81efdef685d0132fc";
