//! The ZIKARON Desk shell. One window, the widget library's skin, async through channels, the UI never
//! waits for the chain.
//!
//! It carries a diagnostic trace channel (trace marks, on in release builds) that records which components
//! ran, plus test hooks (the `drive` feature, off in normal builds).
//!
//! The shell decides nothing: this layer makes no ledger-protocol decision. Every ledger action on the face
//! has an equivalent CLI verb.
//!
//! Design lives in the widget library: no color, corner radius, line height or font size lives here; all
//! are in `zikaron-ui`.
//!
//! Modules:
//! - [`feature`] closed table of component codes (a trace mark's content can only be one)
//! - [`trace`] trace channel
//! - [`fault`] three-way errors (known translated, unknown passed through, evidence tail kept)
//! - [`task`] background tasks and the Outcome channel (single flight per polling round; the UI frame never
//! blocks)
//! - [`probe`] self-check: real disk reads, no invented numbers
//! - [`action`] one owner: every action of the window (and of the test hooks) passes through here
//! - [`shell`] shell state
//! - [`window`] the only place egui lives on the app side

pub mod action;
pub mod anchorx;
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
/// Secret strings: the widget library's type, taken from here on the app side (named apart from
/// `key::Secret`, the private key type).
pub mod secret {
    pub use zikaron_ui::secret::*;
}
pub mod mirror;
pub mod nav;
pub mod payloadx;
pub mod places;
pub mod probe;
pub mod qr;
pub mod queue;
pub mod readerx;
pub mod recordsx;
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

/// The anchor key layer hangs under `sign`, while both files stay where they are (`sign.rs` finds it with
/// `#[path]`).
///
/// This lets the key-lending exit (`key::Secret::with_sign_key`) be `pub(in crate::sign)`: Rust grants
/// visibility only along the module tree, and "visible only to one sibling module" cannot be written, so
/// `key` is made a child of `sign`, the lending exit is visible only to `sign` and its children, and other
/// files in the crate cannot even write the call.
///
/// Both files stay in place: every call site of the signing primitive is in `sign.rs`, and
/// `app::key::generate` is expected to live in `app/src/key.rs`. This re-export keeps `crate::key::…`
/// working, so nothing elsewhere needs to change.
pub use sign::key;
pub mod shell;
pub mod task;
pub mod termsx;
pub mod trace;
pub mod vaultx;
pub mod verifyx;
pub mod watchx;
pub mod window;
pub mod when;
pub mod wizard;
pub mod zlibx;
