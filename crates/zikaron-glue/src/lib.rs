//! Glue behavior for disclosure kits, and the conventions the app and the command line share
//! ([`recording`]: how a `history` entry records bytes).
//!
//! - Kit output ([`pack`], [`select`], [`tidy`]): choose entries, attach files and proof kits, lay out the
//! directory and manifest by kit law §7, self-verify with the kit core, and write only on KIT_OK.
//!
//! This crate decides nothing on its own: kit validity is judged by the kit core's `verify_kit`, grant state
//! by its six checks, ledger completeness by the core's audit, and whether an anchor is on chain by the
//! anchoring crate's scan. It chooses, lays out, asks and passes answers on unchanged. Every public entry
//! point emits the kit output trace mark.

/// The component code of kit output in the diagnostic trace.
pub const V2: &str = "V2";

/// Emit the kit output trace mark at a public entry point.
pub fn seam_v2() {
    zikaron::trace::mark(V2);
}

pub mod container;
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
