//! Anchoring and scanning.
//!
//! Both anchoring forms, the law §9.4 scan that yields the anchor set and the adoption evidence set, endpoint
//! recording and replay, the endpoint rule and proof kits. The output is the third and fifth audit inputs
//! plus the basis, handed to the core for the report.
//!
//! This crate draws no audit conclusion of its own: whether a basis is a `zikaron/1` basis is answered by the
//! core (asked with an empty-pile audit input), and the report and label come from the core's `audit`. This
//! crate reads chain bytes into §9.2 records.
//!
//! RLP, transaction encoding and sender recovery, JSON-RPC transport and MPT proof checking are written here
//! with no third-party chain crates; cryptography goes through the core's `cryptox` only.

/// The component code this crate carries in its trace marks.
pub const A2: &str = "A2";

/// Emit the trace mark when a public entry point is crossed.
///
/// Every public verb (scan, fragment, anchor, assemble, audit, kit capture and verify, endpoint convergence)
/// calls this first, so running any one of them shows up in the diagnostic trace as touching this crate. A
/// new public verb calls it too.
pub fn seam() {
    zikaron::trace::mark(A2);
}

pub mod endpoints;
pub mod input;
pub mod judge;
pub mod kit;
pub mod mpt;
pub mod patience;
pub mod rlp;
pub mod scan;
pub mod send;
pub mod rpc;
pub mod said;
pub mod tx;
pub mod wire;
