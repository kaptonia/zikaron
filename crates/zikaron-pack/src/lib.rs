//! Packaging helpers (see `Cargo.toml`). Each is a function over bytes or files with a named failure; the
//! `zikaron-pack` CLI exposes them to the packaging scripts.

pub mod elf;
pub mod hfs;
pub mod notices;
pub mod paths;
pub mod runtime;
pub mod sha256;
pub mod udif;
pub mod windows;
