//! Ledger storage.
//!
//! Everything a ledger directory does: atomic append (temporary file, fsync, hard link, directory fsync),
//! refusal to overwrite (same bytes idempotent, different bytes refused with nothing changed), strict and
//! lenient reads, an entry size cap, any directory as a valid archive, and a sweep that removes only this
//! crate's own temporary files.
//!
//! This layer does not know the law: entries are opaque bytes. It parses no JSON, computes no digest,
//! verifies no signature, reads no chain and has no third-party dependency.
//!
//! [`LedgerDir::pile`] yields exactly the pile of an audit input: the caller writes it in the transport
//! spelling of `HARNESS.md` (`0x` plus hex) and hands it to the core's `audit`. The pile is always complete;
//! a missing entry is refused, never dropped.

pub mod codes;
pub mod layout;
pub mod ledger;
pub mod trace;

pub use codes::{Code, Trouble, Why};
pub use layout::{EntryName, Kind};
pub use ledger::{LedgerDir, Layout, Pile, Skip, Stored, Survey, Sweep, ENTRY_MAX};
