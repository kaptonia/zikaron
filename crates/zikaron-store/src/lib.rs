//! Ledger storage.
//!
//! Atomic append (temporary file, fsync, hard link, directory fsync), refusal to overwrite (same bytes are
//! idempotent, different bytes are refused with nothing changed), strict and lenient reads, an entry size
//! cap, any directory as a valid archive, and a sweep that removes only this crate's own temporary files.
//!
//! Entries are opaque bytes: this crate parses no JSON, computes no digest, verifies no signature, reads no
//! chain and has no third-party dependency.
//!
//! [`LedgerDir::pile`] yields the pile for an audit input; the caller encodes it as `0x`-prefixed hex (see
//! `HARNESS.md`) and passes it to the core's `audit`. A pile is always complete: a missing entry is refused,
//! never dropped.

pub mod codes;
pub mod layout;
pub mod ledger;
pub mod trace;

pub use codes::{Code, Trouble, Why};
pub use layout::{EntryName, Kind};
pub use ledger::{LedgerDir, Layout, Pile, Skip, Stored, Survey, Sweep, ENTRY_MAX};
