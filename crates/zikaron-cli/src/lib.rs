#![forbid(unsafe_code)]

pub const V1: &str = "V1";

pub fn seam() {
    zikaron::trace::mark(V1);
}

pub mod args;
pub mod chain;
pub mod codes;
pub mod codes_own;
pub mod desk;
pub mod entropy;
pub mod entry;
pub mod faceverbs;
pub mod grantpack;
pub mod handoff;
pub mod hopin;
pub mod issuing;
pub mod kitout;
pub mod ledger;
pub mod mcp;
pub mod mcptools;
pub mod named;
pub mod out;
pub mod sealed;
pub mod sealedsend;
pub use zikaron_send::sent;

pub const GRANT_ORIGINALS_ALL: &str = "all";

pub const CLEAN_YES: &str = "yes";

pub const CLEAN_UNLISTED: &str = "kept";
pub mod verbs;
pub mod verbs_own;
