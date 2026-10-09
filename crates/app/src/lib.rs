//! The ZIKARON Desk app: one window, styled by the `zikaron-ui` widget library, with background work on
//! channels so the UI never waits for the chain.
//!
//! It has a diagnostic trace channel (on in release builds) that records which components ran, and test hooks
//! (the `drive` feature, off in normal builds).
//!
//! The app makes no ledger-protocol decisions itself; those live in the core crates. Every ledger action in
//! the UI has an equivalent CLI command.
//!
//! All visual design (colors, corner radii, line heights, font sizes) lives in `zikaron-ui`, not here.
//!
//! Key modules:
//! - [`feature`] the closed set of component codes a trace mark can carry
//! - [`trace`] trace channel
//! - [`fault`] errors: known ones translated, unknown ones passed through, with the details kept
//! - [`task`] background tasks and the outcome channel (the UI frame never blocks)
//! - [`probe`] self-check from real disk reads
//! - [`action`] every window action (and test hook) goes through here
//! - [`shell`] app state
//! - [`window`] the only app module that uses egui

pub mod action;
pub mod anchorx;
pub mod about;
pub mod adoptx;
pub mod auditx;
pub mod exitgate;
pub mod backup;
pub mod badgex;
pub mod chainx;
pub mod checkx;
pub mod cryptx;
pub mod deliveryx;
pub mod deploy;
pub mod door;
pub mod depthx;
pub mod diligx;
pub mod entryx;
pub mod family;
pub mod fault;
pub mod firstrun;
pub mod gitx;
pub mod grantfilex;
pub mod grantx;
pub mod home;
pub mod identity;
pub mod feature;
pub mod fetchx;
pub mod landing;
pub mod lang;
pub mod lastread;
pub mod ledgerx;
pub mod keybox;
pub mod platform;
pub mod kitx;
pub mod kitsindex;
pub mod keystore;
pub mod local;
pub mod lock;
pub mod machine;
/// Secret strings, re-exported from the widget library (distinct from `key::Secret`, the private key type).
pub mod secret {
    pub use zikaron_ui::secret::*;
}
pub mod mirror;
pub mod nav;
pub mod payloadx;
pub mod pinned;
pub mod places;
pub mod probe;
pub mod qr;
pub mod queue;
pub mod readerx;
pub mod readnets;
pub mod recordsx;
pub mod checkedx;
pub mod restorex;
pub mod names;
pub mod register;
pub mod rekey;
pub mod relicx;
pub mod retractx;
pub mod roles;
pub mod sentinelx;
pub mod settings;
pub mod succeedx;
pub mod supplyx;
pub mod sign;

/// The key module is a child of `sign` (`sign.rs` includes `key.rs` with `#[path]`).
///
/// This lets `key::Secret::with_sign_key`, which lends out the raw signing key, be `pub(in crate::sign)`:
/// Rust visibility follows the module tree and cannot name a single sibling module, so making `key` a child
/// of `sign` keeps every other module in the crate from even writing the call. This re-export keeps
/// `crate::key::…` paths working.
pub use sign::key;
pub mod shell;
pub mod task;
pub mod termsx;
pub mod trace;
pub mod vaultx;
pub mod verifiedx;
pub mod verifyx;
pub mod watchx;
pub mod widex;
pub mod window;
pub mod when;
pub mod wizard;
pub mod zlibx;
