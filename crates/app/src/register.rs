use crate::fault::{Fault, Known};
use crate::identity::{Registry, Row};
use crate::roles::Role;
use std::path::PathBuf;

pub fn path() -> Result<PathBuf, Fault> {
    Ok(crate::home::machine_dir()?.join(crate::places::registry_file()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Watch {
    Register,
}

impl crate::reads::Watched for Watch {
    const ALL: &'static [Watch] = &[Watch::Register];
    fn path(self, machine: &std::path::Path) -> PathBuf {
        match self {
            Watch::Register => machine.join(crate::places::registry_file()),
        }
    }
}

pub fn read() -> Result<Option<Registry>, Fault> {
    crate::reads::touch();
    crate::trace::mark(crate::feature::Feature::H3);
    let p = path()?;
    match crate::local::read(&p, crate::local::Doc::Registry)? {
        Some(b) => {
            let (mut reg, upgraded) = Registry::parse_shaped(&b).map_err(|why| Fault::known(Known::IdentitiesShape, format!("{}: {why}", p.display())))?;
            crate::identity::follow_machine(&mut reg);
            if upgraded {
                let said = write(&reg).err();
                *WRITE_BACK.lock().unwrap_or_else(|e| e.into_inner()) = said;
            }
            Ok(Some(reg))
        }
        None => Ok(None),
    }
}

static WRITE_BACK: std::sync::Mutex<Option<Fault>> = std::sync::Mutex::new(None);

pub fn write_back_trouble() -> Option<Fault> {
    WRITE_BACK.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

pub fn write(reg: &Registry) -> Result<(), Fault> {
    crate::trace::mark(crate::feature::Feature::H3);
    let machine = crate::home::machine_dir()?;
    crate::local::put(&machine, &crate::places::registry_file(), crate::local::Doc::Registry, &reg.to_bytes())?;
    crate::reads::wrote(crate::reads::Copies::Register);
    Ok(())
}

pub fn view() -> Result<Registry, Fault> {
    Ok(crate::identity::view(read()?))
}

pub fn now_row_listed() -> Result<Option<(Row, Role)>, Fault> {
    Ok(read()?.and_then(|r| r.now().map(|(row, s)| (row.clone(), s))))
}

pub fn account_now() -> Result<Option<String>, Fault> {
    Ok(crate::identity::account_now(&view()?))
}

pub fn change_listed<T>(apply: impl FnOnce(&mut Registry) -> Result<T, Fault>) -> Result<Option<T>, Fault> {
    let Some(mut reg) = read()? else { return Ok(None) };
    let before = reg.clone();
    let out = apply(&mut reg)?;
    if reg != before {
        write(&reg)?;
    }
    Ok(Some(out))
}

pub fn change<T>(apply: impl FnOnce(&mut Registry) -> Result<T, Fault>) -> Result<T, Fault> {
    let listed = read()?;
    let had = listed.is_some();
    let mut reg = crate::identity::view(listed);
    let before = reg.clone();
    let out = apply(&mut reg)?;
    if !had || reg != before {
        write(&reg)?;
    }
    Ok(out)
}
