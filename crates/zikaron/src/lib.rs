//! The `zikaron/1` core: every [E] and [C] predicate of law §3 to §9.5, built from the base law text and the
//! reference implementations.
//!
//! - [`json`] canonical form (§3): our own parser and canonicalizer, no third-party JSON.
//! - [`entry`] envelope, signature, the seven body types (§4 to §6).
//! - [`audit`] lineage, the walk, the five-input audit and its report (§7 to §9.5).
//! - [`cryptox`] the one place third-party cryptography is used.
//! - [`hexfmt`] the one place hex is spelled.
//! - [`tokens`] closed sets of law constants (refusal tokens, finding names, labels, verdicts, domains).
//! - [`trace`] trace channel: a trace mark when a public entry point is crossed (diagnostic, never a decision).

pub mod audit;
pub mod cryptox;
pub mod entry;
pub mod hexfmt;
pub mod json;
pub mod tokens;
pub mod trace;
