//! Where this machine keeps the pen of each ledger it has handed over or taken (`handover`, the family's handoff
//! rules): one list in the machine directory ([`FILE`]), each ledger keyed blind as the adopted list keys it,
//! `sha256(salt ‖ root id)` under the machine's one salt (`adopted::salt_made`), so the list names no ledger.
//! The command line (a plain ledger folder, or a home it writes with the credential) and the desktop read it
//! before each write ([`may_write`]); only a handoff step writes it.

use crate::handover::{Pen, Pens};
use crate::trouble::{Code, Trouble};
use std::path::Path;

/// The list's file name, in the machine directory.
pub const FILE: &str = "handed-pens";

/// The key of the ledger whose root (genesis entry) id is `root`.
pub fn key_of(machine_dir: &Path, root: &[u8; 32]) -> Result<[u8; 32], Trouble> {
    Ok(crate::adopted::blind(&crate::adopted::salt_made(machine_dir)?, root))
}

/// The list (empty when there is none). A list out of its shape is refused by name, never read past.
pub fn read(machine_dir: &Path) -> Result<Pens, Trouble> {
    let at = machine_dir.join(FILE);
    match std::fs::read(&at) {
        Ok(b) => Pens::parse(&b).ok_or_else(|| Trouble::new(Code::SettingsShape, at.display().to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Pens::default()),
        Err(e) => Err(Trouble::io(&e, &at.display().to_string())),
    }
}

/// Write the list whole (atomic replacement).
pub fn write(machine_dir: &Path, pens: &Pens) -> Result<(), Trouble> {
    crate::land::put_at(machine_dir, FILE, &pens.render())
}

/// The pen of the ledger whose root id is `root`, on this machine.
pub fn pen(machine_dir: &Path, root: &[u8; 32]) -> Result<Pen, Trouble> {
    // No list yet: no ledger was handed here, and no salt need be made to say so.
    if !machine_dir.join(FILE).exists() {
        return Ok(Pen::Holds { took: None });
    }
    Ok(read(machine_dir)?.pen(&key_of(machine_dir, root)?))
}

/// Whether the ledger whose root id is `root` may be written on this machine: refused (`HandedOff`, the handoff's
/// id) while it is handed over.
pub fn may_write(machine_dir: &Path, root: &[u8; 32]) -> Result<(), Trouble> {
    match crate::handover::may_write(&pen(machine_dir, root)?) {
        Ok(()) => Ok(()),
        Err(crate::handover::Refusal::Handed(id)) => Err(Trouble::new(Code::HandedOff, id.hex())),
        Err(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handover::HandId;

    #[test]
    fn a_handed_ledger_is_refused_by_its_handoff_and_the_list_names_no_ledger() {
        let d = std::env::temp_dir().join(format!("zk-seal-pens-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("dir");
        let root = [7u8; 32];
        assert_eq!(may_write(&d, &root), Ok(()), "无表即持笔");
        let mut p = read(&d).expect("empty");
        p.set(&key_of(&d, &root).expect("key"), Pen::Handed { id: HandId([3; 16]), after: None });
        write(&d, &p).expect("written");
        assert_eq!(may_write(&d, &root).err().map(|t| (t.code, t.tail)), Some((Code::HandedOff, HandId([3; 16]).hex())));
        assert_eq!(may_write(&d, &[8u8; 32]), Ok(()), "别的账照写");
        let text = std::fs::read_to_string(d.join(FILE)).expect("list");
        assert!(!text.contains(&"07".repeat(32)), "表里不记根");
        std::fs::write(d.join(FILE), b"bad\n").expect("bad");
        assert_eq!(may_write(&d, &root).err().map(|t| t.code), Some(Code::SettingsShape), "表坏即具名拒");
        let _ = std::fs::remove_dir_all(&d);
    }
}
