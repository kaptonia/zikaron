//! The packaging pieces (see `Cargo.toml`). Each one is a function over bytes or files with a named failure;
//! the `zikaron-pack` command line puts them in reach of the packaging scripts.

pub mod elf;
pub mod hfs;
pub mod notices;
pub mod udif;
pub mod windows;
