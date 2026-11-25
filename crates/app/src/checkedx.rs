use crate::fault::Fault;
use zikaron_anchor::scan::{Fact, FactKey, Known};

pub use crate::sendx::record::{agreed, is_logs, is_proof, DIR, FILE, FORM, LOGS};

pub fn settled_keeper() -> std::sync::Arc<dyn zikaron_anchor::settled::Keep> {
    crate::sendx::record::settled_keeper()
}

pub fn read() -> Known {
    crate::sendx::record::facts()
}

pub fn add(new: &[(FactKey, Fact)]) -> Result<usize, Fault> {
    crate::sendx::record::add(new)
}

pub fn read_single() -> Known {
    crate::sendx::record::facts_single()
}

pub fn add_single(new: &[(FactKey, Fact)]) -> Result<usize, Fault> {
    crate::sendx::record::add_single(new)
}

pub fn forget() -> Result<bool, Fault> {
    let _turn = crate::sendx::record::TURN.lock().unwrap_or_else(|e| e.into_inner());
    let _settled = zikaron_anchor::settled::turn();
    let machine = crate::home::machine_dir()?;
    let dir = machine.join(DIR);
    let _held = [FILE, LOGS].map(|file| crate::sendx::record::hold_at(&dir, file));
    let mut had = crate::sendx::forget_sent_logs(&machine)?;
    let p = dir.join(FILE);
    match std::fs::remove_file(&p) {
        Ok(()) => had = true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(crate::fault::classify(&e, &p.display().to_string())),
    }
    had |= crate::sendx::record::forget_logs_in(&dir).map_err(|e| crate::fault::classify(&e, &dir.display().to_string()))?;
    Ok(had)
}

pub fn count() -> usize {
    read().0.len()
}
