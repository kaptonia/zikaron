use crate::handover::{Pen, Pens};
use crate::trouble::{Code, Trouble};
use std::path::Path;

pub const FILE: &str = "handed-pens";

pub fn key_of(machine_dir: &Path, root: &[u8; 32]) -> Result<[u8; 32], Trouble> {
    Ok(crate::adopted::blind(&crate::adopted::salt_made(machine_dir)?, root))
}

pub fn read(machine_dir: &Path) -> Result<Pens, Trouble> {
    let at = machine_dir.join(FILE);
    match std::fs::read(&at) {
        Ok(b) => Pens::parse(&b).ok_or_else(|| Trouble::new(Code::SettingsShape, at.display().to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Pens::default()),
        Err(e) => Err(Trouble::io(&e, &at.display().to_string())),
    }
}

pub fn write(machine_dir: &Path, pens: &Pens) -> Result<(), Trouble> {
    crate::land::put_at(machine_dir, FILE, &pens.render())
}

pub fn pen(machine_dir: &Path, root: &[u8; 32]) -> Result<Pen, Trouble> {
    if !machine_dir.join(FILE).exists() {
        return Ok(Pen::Holds { took: None });
    }
    Ok(read(machine_dir)?.pen(&key_of(machine_dir, root)?))
}

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
