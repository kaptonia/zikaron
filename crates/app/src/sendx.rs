use crate::fault::Fault;
use crate::home::Home;
use crate::queue::Kept;

pub struct Desk {
    home: Home,
    writer: bool,
}

impl Desk {
    pub fn open(root: &std::path::Path) -> Result<Desk, Fault> {
        crate::chainx::speak();
        Ok(Desk { home: Home::open(root)?, writer: false })
    }

    pub fn of(home: &Home) -> Desk {
        crate::chainx::speak();
        Desk { home: Home::bare(home.root().to_path_buf()), writer: false }
    }
}

pub const NOT_INCLUDED: &str = zikaron_send::withdraw::NOT_INCLUDED;

pub fn write_gates(root: &std::path::Path) -> Result<(), Fault> {
    zikaron_send::shut::check(&Desk::open(root)?).map_err(Fault::gate_owned)
}

pub use zikaron_send::shut::standing as mark_standing;

pub fn lift_met(root: &std::path::Path) -> Result<(), Fault> {
    let mut d = Desk::open(root)?;
    d.writer = true;
    zikaron_send::shut::lift(&d)
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

    fn behind(&self, lacks: &[String]) -> Fault {
        let state = crate::restorex::State::lacking(lacks);
        match crate::restorex::write(&self.home, &state) {
            Ok(()) => crate::restorex::fault(&state).worded(Some(crate::lang::Key::ExitBehind), Some(crate::lang::Key::ExitBehindSay)),
            Err(f) => f,
        }
    }

    fn unread(&self, f: Fault) -> Fault {
        f.worded(None, Some(crate::lang::Key::ExitRetryLater))
    }

    fn mark(&self) -> Result<Option<crate::restorex::State>, Fault> {
        crate::restorex::read(&self.home)
    }

    fn lift(&self) -> Result<(), Fault> {
        if self.writer {
            crate::restorex::clear(&self.home)?;
        }
        Ok(())
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

impl zikaron_send::Facts for Desk {
    fn known(&self) -> zikaron_anchor::scan::Known {
        zikaron_send::Facts::known(&Local)
    }

    fn known_single(&self) -> zikaron_anchor::scan::Known {
        zikaron_send::Facts::known_single(&Local)
    }

    fn add(&self, runs: &[(Vec<u64>, zikaron_anchor::scan::Sightings)], thin: &[u64]) {
        zikaron_send::Facts::add(&Local, runs, thin)
    }
}

pub use zikaron_send::checked::Notice;
pub use zikaron_send::issue::{member_of, Member};
pub use zikaron_send::withdraw::standing;
pub use zikaron_send::withdraw::{judged, Back};
pub use zikaron_send::withdraw::unsettled;

pub fn keep_noticed(eps: &[crate::chainx::Endpoint], pile: &[Vec<u8>]) -> usize {
    let found = zikaron_send::withdraw::unrecorded(eps, pile, &record::noticed());
    found.into_iter().filter(|n| record::add_noticed(n.clone()).unwrap_or(false)).count()
}

pub use zikaron_send::behind;

pub mod record {
    use crate::fault::Fault;
    pub use zikaron_send::checked::{agreed, audit_file, forget_logs_in, hold_at, is_logs, logs_file, Room, DIR, FILE, FORM, LOGS, TURN};
    pub use zikaron_send::proofs::is_proof;

    pub struct Desk;

    impl zikaron_send::checked::FailSaid for Fault {
        fn said(&self) -> String {
            self.raw()
        }
        fn locked(&self) -> bool {
            self.which() == Some(crate::fault::Known::Locked)
        }
    }

    impl zikaron_send::checked::Room for Desk {
        type Fail = Fault;

        fn read(&self, file: &str) -> Option<Vec<u8>> {
            let p = crate::home::machine_dir().ok()?.join(DIR).join(file);
            crate::local::read(&p, crate::local::Doc::Checked).ok().flatten()
        }

        fn put(&self, file: &str, plain: &[u8]) -> Result<(), Fault> {
            let machine = crate::home::machine_dir()?;
            crate::local::put(&machine.join(DIR), file, crate::local::Doc::Checked, plain)
        }

        fn hold(&self, file: &str) -> Option<zikaron_os::LockFile> {
            hold_at(&crate::home::machine_dir().ok()?.join(DIR), file)
        }

        fn files(&self) -> Vec<String> {
            crate::home::machine_dir().map(|m| zikaron_send::checked::files_in(&m.join(DIR))).unwrap_or_default()
        }

        fn remove(&self, file: &str) {
            if let Ok(m) = crate::home::machine_dir() {
                let _ = std::fs::remove_file(m.join(DIR).join(file));
            }
        }
    }

    pub fn facts() -> zikaron_anchor::scan::Known {
        zikaron_send::checked::facts(&Desk)
    }

    pub fn add(new: &[(zikaron_anchor::scan::FactKey, zikaron_anchor::scan::Fact)]) -> Result<usize, Fault> {
        zikaron_send::checked::add(&Desk, new)
    }

    pub fn facts_single() -> zikaron_anchor::scan::Known {
        zikaron_send::checked::facts_single(&Desk)
    }

    pub fn add_single(new: &[(zikaron_anchor::scan::FactKey, zikaron_anchor::scan::Fact)]) -> Result<usize, Fault> {
        zikaron_send::checked::add_single(&Desk, new)
    }

    pub fn noticed() -> Vec<zikaron_send::checked::Notice> {
        zikaron_send::checked::noticed(&Desk)
    }

    pub fn add_noticed(n: zikaron_send::checked::Notice) -> Result<bool, Fault> {
        let added = zikaron_send::checked::add_noticed(&Desk, n)?;
        if added {
            crate::reads::wrote(crate::reads::Copies::Noticed);
        }
        Ok(added)
    }

    pub fn settled_keeper() -> std::sync::Arc<dyn zikaron_anchor::settled::Keep> {
        match key_now() {
            Some(key) => std::sync::Arc::new(KeptAt { key, logs: zikaron_send::checked::Logs(Desk) }),
            None => std::sync::Arc::new(zikaron_send::checked::Logs(Desk)),
        }
    }

    fn key_now() -> Option<[u8; 32]> {
        crate::keybox::local_key().ok().map(|k| zikaron::cryptox::sha256(k.bytes()))
    }

    struct KeptAt {
        key: [u8; 32],
        logs: zikaron_send::checked::Logs<Desk>,
    }

    impl KeptAt {
        fn moved(&self) -> bool {
            key_now().is_some_and(|k| k != self.key)
        }
    }

    impl zikaron_anchor::settled::Keep for KeptAt {
        fn load(&self, part: &str) -> Option<Vec<u8>> {
            if self.moved() {
                return None;
            }
            zikaron_anchor::settled::Keep::load(&self.logs, part)
        }

        fn save(&self, part: &str, plain: &[u8]) -> Result<(), zikaron_anchor::settled::Unkept> {
            if self.moved() {
                return Err(zikaron_anchor::settled::Unkept::KeyChanged);
            }
            zikaron_anchor::settled::Keep::save(&self.logs, part, plain)
        }

        fn parts(&self) -> Vec<String> {
            zikaron_anchor::settled::Keep::parts(&self.logs)
        }

        fn hold(&self) -> Option<Box<dyn std::any::Any>> {
            zikaron_anchor::settled::Keep::hold(&self.logs)
        }

        fn public(&self) -> bool {
            zikaron_anchor::settled::Keep::public(&self.logs)
        }
    }
}

pub struct Local;

impl zikaron_send::Facts for Local {
    fn known(&self) -> zikaron_anchor::scan::Known {
        zikaron_send::Facts::known(&zikaron_send::checked::Record(record::Desk))
    }

    fn known_single(&self) -> zikaron_anchor::scan::Known {
        zikaron_send::Facts::known_single(&zikaron_send::checked::Record(record::Desk))
    }

    fn add(&self, runs: &[(Vec<u64>, zikaron_anchor::scan::Sightings)], thin: &[u64]) {
        zikaron_send::Facts::add(&zikaron_send::checked::Record(record::Desk), runs, thin)
    }
}

pub fn proofs_beside(pile: &[Vec<u8>], ground: &zikaron_seal::ground::Ground, package: &std::path::Path) -> Result<usize, crate::fault::Fault> {
    let root = crate::auditx::root_of(pile)?;
    let root: [u8; 20] = zikaron::hexfmt::decode(&root).and_then(|b| b.try_into().ok()).ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoGenesis, root.clone()))?;
    let senders = zikaron_send::proofs::senders_from(&crate::auditx::senders_of(pile));
    let proofs = zikaron_send::proofs::for_export(&record::Desk, &senders, ground, &crate::chainx::endpoint_at);
    zikaron_glue::anchorproofs::write_beside(package, &root, &proofs).map_err(crate::fault::Fault::of_landing)?;
    Ok(proofs.len())
}

pub fn forget_sent_logs(machine: &std::path::Path) -> Result<bool, crate::fault::Fault> {
    zikaron_send::sent::forget_logs(machine).map_err(|e| crate::fault::classify(&e, &machine.join(zikaron_send::sent::DIR).display().to_string()))
}
