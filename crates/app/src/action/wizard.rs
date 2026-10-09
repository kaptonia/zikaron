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
    // The checklist writes into this home, so the entry write gate applies (read-only instance, broken chain,
    // handed over); otherwise a second instance could overwrite the writer's checklist.
    shell.may_write_entries()?;
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    // The shell's copy is this home's checklist: loaded when the home opens (`Shell::hydrate`) and updated in
    // place on each tick, so the disk is not reread. The write gate already guarantees a single writer.
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
