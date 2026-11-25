#![forbid(unsafe_code)]

pub const A2: &str = "A2";

pub fn seam() {
    zikaron::trace::mark(A2);
}

pub mod action;
pub mod ahead;
pub mod book;
pub mod deploy;
pub mod endpoints;
pub mod input;
pub mod judge;
pub mod oneread;
pub mod patience;
pub mod proofs;
pub mod rest;
pub mod revoke;
pub mod rlp;
pub mod scan;
pub mod send;
pub mod settled;
pub mod rpc;
pub mod said;
pub mod trie;
pub mod tx;
pub mod wire;
