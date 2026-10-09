//! The `zikaron.kit/1` core: the client-side document and reading spec (kit law §3 to §10,
//! `base/zikaron-kit-v1.md`), built on the `zikaron/1` core (kit law §13.4). Canonical form, acceptance,
//! signatures and audit go through the public API of `zikaron`; this crate has no third-party cryptography.
//!
//! - [`doc`] two-domain documents, pairing and attribution (§3 to §5).
//! - [`badge`] grant payload encoding and the byte link (§6).
//! - [`kitdir`] disclosure kit enumeration, manifest and verification (§7).
//! - [`reading`] reach, anchoring and the depth reading (§8, §9).
//! - [`check`] six checks, ledger link and chain check (§10).
//! - [`b64`] the single base64url implementation.
//! - [`tokens`] the closed constants of kit law §11.

pub mod b64;
pub mod badge;
pub mod check;
pub mod doc;
pub mod kitdir;
pub mod reading;
pub mod tokens;
