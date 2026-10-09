//! The `zikaron` command line: twenty-two verbs; with `--home`, ten of them are done by the running desktop
//! through its local IPC endpoint ([`desk`]).
//!
//! The verbs are a thin shell: entries are built and checked by the core, directories read and written by the
//! storage crate, chains scanned and written by the anchoring crate, documents, payloads, kits, depth and
//! grant checks produced by the kit core. This crate makes no law decision: it arranges arguments, calls the
//! public APIs, and renders the state they return on one line.
//!
//! 1. Nothing bypasses the core: entry bytes come only from the core's `json::canon_bytes`, signatures only
//! from its `presig_and_digest` and `cryptox::sign_digest`, and every entry passes `entry::check` before
//! landing. Directories are never written directly (storage crate `LedgerDir` only); payloads are never
//! assembled here (kit core only).
//! 2. States pass through unchanged: GREEN / PARTIAL / FAIL, COMPLETE / GAPS / UNAVAILABLE / BROKEN_CHAIN,
//! KIT_OK and every token are bytes from below, never merged or rewritten. PARTIAL is its own state with its
//! own exit code ([`codes::Exit::Partial`]).
//! 3. No answer and a negative answer are separate (law §9.4): an unreachable endpoint, disagreeing readings
//! or a declined scan is the absence of an answer ([`codes::Exit::Unanswered`]); only a failing subject is a
//! negative answer ([`codes::Exit::Denied`]).

/// The component code this crate carries in its trace marks.
pub const V1: &str = "V1";

/// Emit the trace mark at a public entry point.
///
/// Every public verb calls this first, so running any one of them shows up in the diagnostic trace as touching
/// this crate. A new verb calls it too.
pub fn seam() {
    zikaron::trace::mark(V1);
}

pub mod args;
pub mod chain;
pub mod codes;
pub mod desk;
pub mod docs;
pub mod entropy;
pub mod entry;
pub mod kitout;
pub mod ledger;
pub mod out;
pub mod sent;
pub mod verbs;
