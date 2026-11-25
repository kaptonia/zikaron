use super::*;

pub(super) fn ground_bare(shell: &Shell) -> Result<crate::auditx::Ground, crate::fault::Fault> {
    let chain = shell.settings.chain_id.ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoChainId, String::new())
    })?;
    let registry = shell.settings.registry.ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoRegistry, String::new())
    })?;
    Ok(crate::auditx::Ground {
        chain,
        registry,
        from_block: shell.settings.from_block,
        to_block: shell.settings.from_block,
        senders: Vec::new(),
        earlier: Vec::new(),
    })
}

pub(super) fn to_head(
    eps: &[crate::chainx::Endpoint],
    mut g: crate::auditx::Ground,
) -> Result<crate::auditx::Ground, crate::fault::Fault> {
    let (head, _) = crate::chainx::head_block(eps, g.chain)?;
    g.to_block = head.max(g.from_block);
    Ok(g)
}

pub(super) fn diligence(
    shell: &mut Shell,
    address: &str,
    dir: &str,
    work: &str,
    from: &str,
    to: &str,
) -> Result<Spawned, crate::fault::Fault> {
    let who = crate::readerx::who(address)?;
    let dir = one_place(dir)?;
    let work = work.trim().to_string();
    if !work.is_empty() {
        crate::verifyx::work_of(&work)?;
    }
    let window = window_of(from, to)?;
    let g = ground_bare(shell)?;
    let eps = shell.endpoints.clone();
    let nets = read_nets_now()?;
    if crate::readnets::all_endpoints(&eps, &nets).is_empty() {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::NoEndpoint,
            crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail055).to_string()),
        ));
    }
    let shelf = shelf_of(shell, dir);
    shell.diligence = None;
    shell.others_asked = Some(super::reader::OthersAsk { diligence: true, who, eps: eps.clone(), ground: g.clone(), nets: nets.clone(), work: work.clone(), window });
    Ok(shell.tasks.spawn(Kind::Diligence, move || {
        let mut found = crate::supplyx::find_book(&shelf, &who.hex());
        crate::readerx::copy_under_book(&mut found, &who, &crate::readerx::book_offer(Some((&eps, &g)), &nets));
        let bytes = found.supply.as_ref().map(|s| s.items.clone()).unwrap_or_default();
        crate::task::stage_at(Kind::Diligence, 0);
        let book = crate::readerx::read_copy(&who, &bytes)?;
        let mut r = crate::diligx::assemble(book, &crate::auditx::empty_fragment(), &bytes, &work, window)?;
        r.from = found.supply.as_ref().map(|s| (s.level, s.place.clone()));
        r.files = found.supply.as_ref().and_then(|s| s.files);
        r.misses = found.misses;
        r.missed = Vec::new();
        r.now = None;
        r.latest = None;
        r.label = String::new();
        r.audit_key = crate::readerx::audit_key(&format!("diligence:{work}:{window:?}"), &who.hex(), &bytes, Some(&g), &nets);
        r.pile = bytes;
        Ok(Done::Diligence(Box::new(r)))
    }))
}

pub(super) fn save_snapshot(shell: &mut Shell, to: &str) -> Result<usize, crate::fault::Fault> {
    let to = crate::home::landing(to)?;
    let r = match snapshot_of(shell) {
        Snapshot::Held(r) => r,
        Snapshot::Unaudited => return Err(not_audited()),
        Snapshot::NotHeld => return Err(whole_not_held()),
    };
    crate::diligx::snapshot(&r, &to)
}

pub enum Snapshot {
    Held(crate::diligx::Read),
    Unaudited,
    NotHeld,
}

pub fn snapshot_of(shell: &Shell) -> Snapshot {
    let Some(key) = shell.diligence.as_ref().map(|r| r.audit_key) else { return Snapshot::Unaudited };
    let whole = shell.others_whole.lock().unwrap_or_else(|e| e.into_inner()).clone();
    match whole.filter(|(k, _)| *k == key) {
        Some((_, r)) => Snapshot::Held(r),
        None if matches!(shell.audits.state(&key), zikaron_send::behind::State::Done(_)) => Snapshot::NotHeld,
        None => Snapshot::Unaudited,
    }
}

fn whole_not_held() -> crate::fault::Fault {
    crate::fault::Fault::known(crate::fault::Known::NotAudited, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::SnapshotWholeGone).to_string())).worded(Some(crate::lang::Key::SnapshotWholeGone), Some(crate::lang::Key::SnapshotWholeGoneNext))
}

fn not_audited() -> crate::fault::Fault {
    crate::fault::Fault::known(crate::fault::Known::NotAudited, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail056).to_string())).worded(Some(crate::lang::Key::OthersNotAudited), Some(crate::lang::Key::OthersNotAuditedNext))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_snapshot_before_the_audit_points_at_this_pages_audit() {
        use crate::lang::{t, Key};
        let f = super::not_audited();
        assert_eq!(f.which(), Some(crate::fault::Known::NotAudited));
        assert_eq!((f.human(), f.next()), (t(Key::OthersNotAudited), t(Key::OthersNotAuditedNext).to_string()));
        let own = crate::fault::Fault::known(crate::fault::Known::NotAudited, String::new());
        assert_eq!((own.human(), own.next()), (t(Key::FaultWhatNotAudited), t(Key::FaultNextNotAudited).to_string()), "this desk's own ledger's words unchanged");
    }
}
