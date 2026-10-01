use super::*;

pub(super) fn read_depth(shell: &mut Shell, work: &str) -> Result<Spawned, crate::fault::Fault> {
    let want = crate::depthx::work_of(work)?;
    let root = shell
        .home
        .as_ref()
        .map(|h| h.root().to_path_buf())
        .ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    // With an online audit done, use its fragment (depth is relative to the declared basis); otherwise take
    // the offline empty basis, and the reading carries its own label, so "relative to which records" is
    // visible.
    let fragment = match shell.audit.as_ref() {
        Some(a) => a.fragment.clone(),
        None => crate::auditx::empty_fragment(),
    };
    Ok(shell.tasks.spawn(Kind::Depth, move || {
        crate::task::stage_at(Kind::Depth, 0);
        let home = crate::home::Home::open(&root)?;
        let pile = home
            .ledger()?
            .pile()?;
        let three = crate::depthx::read(&pile.items, &fragment, &want)?;
        Ok(Done::Depth { work: want, value: three.value })
    }))
}
