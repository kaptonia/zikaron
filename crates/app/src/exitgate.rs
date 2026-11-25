use crate::fault::Fault;
use std::path::PathBuf;

pub use zikaron_send::gate::{Ask, Exit, Lane, Pass, Reading};

pub fn ask_of(shell: &crate::shell::Shell) -> Result<Ask, Fault> {
    let root = shell.home.as_ref().map(|h| h.root().to_path_buf()).ok_or_else(|| Fault::known(crate::fault::Known::NoHome, String::new()))?;
    ask_from(root, &shell.settings, &shell.endpoints, shell.anchor.map(|a| a.hex())).map_err(Fault::gate_owned)
}

pub fn ask_for_home(home: &crate::home::Home, own: Option<String>) -> Result<Ask, Fault> {
    let s = crate::settings::Settings::read(home)?;
    let eps = crate::chainx::nodes_of(&s);
    ask_from(home.root().to_path_buf(), &s, &eps, own)
}

fn ask_from(root: PathBuf, s: &crate::settings::Settings, eps: &[crate::chainx::Endpoint], own: Option<String>) -> Result<Ask, Fault> {
    Ok(zikaron_send::gate::ask(root, &s.ground(), eps, own)?)
}

pub fn read(ask: &Ask) -> Result<Reading, Fault> {
    zikaron_send::gate::read(ask, &crate::sendx::Desk::open(&ask.root)?)
}

pub fn tail(ask: &Ask) -> Result<crate::restorex::Tail, Fault> {
    let (lacks, anchors) = zikaron_send::gate::tail(ask, &crate::sendx::Desk::open(&ask.root)?)?;
    let missing = lacks.len();
    Ok(if missing == 0 { crate::restorex::Tail::Pass { anchors } } else { crate::restorex::Tail::NewerElsewhere { missing, anchors, lacks } })
}

pub fn pass(ask: &Ask) -> Result<Pass, Fault> {
    let desk = crate::sendx::Desk::open(&ask.root).map_err(|f| f.worded(None, Some(crate::lang::Key::ExitRetryLater)))?;
    zikaron_send::gate::pass(ask, &desk).map_err(Fault::gate_owned)
}
