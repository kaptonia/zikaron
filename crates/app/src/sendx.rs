//! The sender's wiring for this desktop (`zikaron_send`): a home as the sender sees it ([`Desk`]): its queue file
//! read and written under this machine's seal (`queue::Kept`), its ledger, its read-only mark and this machine's
//! folder for the write gates, this desktop's words for the exit gate's two refusals and the read-only mark it
//! places, the running send's stage and this crate's trace mark,
//! and this machine's record of checked chain facts (`checkedx`). The key is lent through `sign` (its
//! `zikaron_send::batch::Signer`).

use crate::fault::Fault;
use crate::home::Home;
use crate::queue::Kept;

/// One home, for the sender.
pub struct Desk {
    home: Home,
}

impl Desk {
    /// The home at `root`, which must be there (`Home::open`).
    pub fn open(root: &std::path::Path) -> Result<Desk, Fault> {
        crate::chainx::speak();
        Ok(Desk { home: Home::open(root)? })
    }

    /// A home already open.
    pub fn of(home: &Home) -> Desk {
        crate::chainx::speak();
        Desk { home: Home::bare(home.root().to_path_buf()) }
    }
}

impl zikaron_send::End for Desk {
    type Fail = Fault;

    fn code_of(f: &Fault) -> Option<String> {
        f.which().map(|k| k.as_str().to_string())
    }

    fn read_queue(&self) -> Result<crate::queue::Queue, Fault> {
        crate::queue::Queue::read(&self.home)
    }

    fn write_queue(&self, q: &crate::queue::Queue) -> Result<(), Fault> {
        q.write(&self.home)
    }

    fn pile(&self) -> Result<Vec<Vec<u8>>, Fault> {
        Ok(self.home.ledger()?.pile()?.items)
    }

    /// The home takes the "newer entries elsewhere" mark (the way to fetching what is missing); a mark that
    /// cannot be written is the refusal instead, with its cause.
    fn behind(&self, missing: usize) -> Fault {
        let state = crate::restorex::State::NewerElsewhere { missing };
        match crate::restorex::write(&self.home, state) {
            Ok(()) => crate::restorex::fault(state).worded(Some(crate::lang::Key::ExitBehind), Some(crate::lang::Key::ExitBehindSay)),
            Err(f) => f,
        }
    }

    fn unread(&self, f: Fault) -> Fault {
        f.worded(None, Some(crate::lang::Key::ExitRetryLater))
    }

    fn mark(&self) -> Result<Option<crate::restorex::State>, Fault> {
        crate::restorex::read(&self.home)
    }

    fn machine(&self) -> Result<std::path::PathBuf, Fault> {
        crate::home::machine_dir()
    }

    fn note(&self, n: zikaron_send::Note) {
        match n {
            zikaron_send::Note::Stage(at) => crate::task::stage_at(crate::task::Kind::Anchor, at),
            zikaron_send::Note::Balance => crate::trace::mark(crate::feature::Feature::H4),
        }
    }
}

/// A home's scans read and keep this machine's record of checked chain facts ([`Local`]).
impl zikaron_send::Facts for Desk {
    fn known(&self) -> zikaron_anchor::scan::Known {
        zikaron_send::Facts::known(&Local)
    }

    fn add(&self, runs: &[(Vec<u64>, zikaron_anchor::scan::Sightings)], thin: &[u64]) {
        zikaron_send::Facts::add(&Local, runs, thin)
    }
}

/// This machine's record of checked chain facts (`checkedx`): read for every scan, and the facts the runs read
/// alike kept.
pub struct Local;

impl zikaron_send::Facts for Local {
    fn known(&self) -> zikaron_anchor::scan::Known {
        crate::checkedx::read()
    }

    fn add(&self, runs: &[(Vec<u64>, zikaron_anchor::scan::Sightings)], thin: &[u64]) {
        let _ = crate::checkedx::add(&crate::checkedx::agreed(runs, thin));
    }
}
