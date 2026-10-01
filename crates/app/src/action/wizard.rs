use super::*;

pub(super) fn wizard_tick(
    shell: &mut Shell,
    step: &str,
    said: &str,
) -> Result<(&'static str, Option<&'static str>), crate::fault::Fault> {
    let s = crate::wizard::Step::parse(step.trim()).ok_or_else(|| {
        crate::fault::Fault::known(
            crate::fault::Known::StepSkipped,
            crate::lang::filln(crate::lang::Key::Tail046, &[&format!("{:?}", step.trim())]),
        )
    })?;
    // The checklist also writes bytes into this home. Read-only instance, broken chain and handed over may
    // not write: with no gate on this path, a second instance could overwrite the writer's checklist with one
    // press.
    shell.may_write_entries()?;
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    // The shell's copy is this home's checklist. It is loaded when the home opens (`Shell::hydrate`) and
    // updated in place when ticked, so this does not read the disk again.
    //
    // Reading the disk on every tick would guard against no second writer (the writing side has one instance,
    // behind the lock and broken-chain gates), and it would add another consumer of "read it back", leaving
    // "close and reopen and stay where you were" without a place of its own in the record.
    let mut w = shell.wizard.clone();
    let done = w.tick(s, said)?;
    w.write(home)?;
    shell.wizard = w;
    Ok((done.as_str(), shell.wizard.next().map(|x| x.as_str())))
}

pub(super) fn wizard_reset(shell: &mut Shell) -> Result<(), crate::fault::Fault> {
    shell.may_write_entries()?;
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let mut w = shell.wizard.clone();
    w.clear();
    w.write(home)?;
    shell.wizard = w;
    Ok(())
}
