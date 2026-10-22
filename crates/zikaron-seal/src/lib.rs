//! The sealed layer of ZIKARON Desk, one implementation for the desktop and the command line: the envelope
//! every local file is sealed in and the names those files take on disk (`envelope`, `names`), the key vault
//! (`keybox`, with keystore V3's parameters in `kdf`), a record package addressed to one key (`addressed`), a home's sealed ledger (`ledger`), what a home's settings
//! and the machine settings say to both ends (`ground`, `machine`), a home's read-only mark (`unfetched`), the machine's read-only networks (`readnets`), and
//! writing a file whole (`land`, over the platform piece). The desktop's `app` crate keeps only its wiring: where the machine
//! directory is, its fault table and words, its trace marks.
//!
//! No window library reaches here, and no sentence for people: refusals are a closed code with evidence
//! (`trouble`), said by each end in its own words. Secrets in hand are wiped through the platform piece's one
//! wipe (`zikaron_os::wipe`).

#![forbid(unsafe_code)]

pub mod addressed;
pub mod adopted;
pub mod credential;
pub mod crypto;
pub mod envelope;
pub mod ground;
pub mod handover;
pub mod identities;
pub mod kdf;
pub mod keybox;
pub mod land;
pub mod ledger;
pub mod machine;
pub mod names;
pub mod pens;
pub mod places;
pub mod queue;
pub mod readnets;
pub mod records;
pub mod terms;
pub mod trouble;
pub mod unfetched;
pub mod writer;

pub use trouble::{Code, Coded, Trouble};
