//! Glue for disclosure kits, and conventions shared by the app and the CLI ([`recording`]: how a `history`
//! entry records bytes).
//!
//! - Kit output ([`pack`], [`select`], [`tidy`]): choose entries, attach files and proof kits, lay out the
//! directory and manifest per kit law §7, self-verify with the kit core, and write only on KIT_OK.
//!
//! This crate makes no judgments of its own: kit validity comes from the kit core's `verify_kit`, grant state
//! from its six checks, ledger completeness from the core's audit, and on-chain anchoring from the anchoring
//! crate's scan; answers are passed on unchanged. Every public entry point emits the kit output trace mark.

/// The component code of kit output in the diagnostic trace.
pub const V2: &str = "V2";

/// Emit the kit output trace mark at a public entry point.
pub fn seam_v2() {
    zikaron::trace::mark(V2);
}

pub mod container;
pub mod door;
pub mod grantfile;
pub mod landing;
pub mod mirror;
pub mod names;
pub mod pack;
pub mod read;
pub mod recording;
pub mod retraction;
pub mod sealed;
pub mod select;
pub mod tidy;
