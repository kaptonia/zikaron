//! Disclosure kits: this layer only hands over.
//!
//! Layout, digests, manifest, junk cleaning and self-verification belong to `zikaron_glue::pack`. This module
//! is a named entry point so the command surface reaches kit output through one place.

use std::path::Path;

/// Kit ingredients, the kit output crate's own type.
pub use zikaron_glue::pack::Bundle;
/// Kit output failures, the kit output crate's own type.
pub use zikaron_glue::pack::Trouble;
/// The landing reading, the kit output crate's own type.
pub use zikaron_glue::pack::Landed;

/// Write a kit: lay out, self-verify, land only on KIT_OK. Judging and layout happen in the kit output crate.
pub fn export(out: &Path, b: Bundle) -> Result<Landed, Trouble> {
    crate::seam();
    zikaron_glue::pack::export(out, b)
}
