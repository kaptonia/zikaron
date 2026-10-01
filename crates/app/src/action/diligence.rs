use super::*;

/// This desk's basis now, without asking the local ledger. Diligence reads someone else's ledger, with
/// senders from that ledger's own lineage (see `readerx::basis_for`); whether the local ledger has a genesis
/// is irrelevant, so only the three cells need to be configured. `toBlock` is set to the lower bound first,
/// and the upper bound is asked of the chain by the background pass (`to_head`): a fixed number would set the
/// same height for every chain, and a scan ending at the start block would find nothing.
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
    })
}

/// Move the basis's upper bound to the chain's latest block now (the smallest across endpoints).
pub(super) fn to_head(
    eps: &[crate::chainx::Endpoint],
    mut g: crate::auditx::Ground,
) -> Result<crate::auditx::Ground, crate::fault::Fault> {
    let (head, _) = crate::chainx::head_block(eps, g.chain)?;
    g.to_block = head.max(g.from_block);
    Ok(g)
}

/// Run one diligence pass. Runs on a background thread.
pub(super) fn diligence(
    shell: &mut Shell,
    address: &str,
    dir: &str,
    work: &str,
    from: &str,
    to: &str,
) -> Result<Spawned, crate::fault::Fault> {
    let who = crate::readerx::who(address)?;
    let work = work.trim().to_string();
    if !work.is_empty() {
        crate::depthx::work_of(&work)?;
    }
    let window = window_of(from, to)?;
    let g = ground_bare(shell)?;
    let eps = shell.endpoints.clone();
    if eps.is_empty() {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::NoEndpoint,
            crate::lang::t(crate::lang::Key::Tail055).to_string(),
        ));
    }
    // Bytes go through the four levels (the same `supplyx::find_book` as the reader and check page): this
    // machine, vault, record bundle, publish address; the "record content" cell is the person's place for the
    // last two, and even when empty the first two are tried. None at all means "not obtained", with each
    // failing level named.
    let shelf = shelf_of(shell, dir.trim());
    shell.diligence = None;
    Ok(shell.tasks.spawn(Kind::Diligence, move || {
        let found = crate::supplyx::find_book(&shelf, &who.hex());
        let bytes = found.supply.as_ref().map(|s| s.items.clone()).unwrap_or_default();
        // The fragment is scanned once: label, anchor count and quantities all speak of the same pass.
        crate::task::stage_at(Kind::Diligence, 0);
        let g = crate::readerx::basis_for(&to_head(&eps, g)?, &who, &bytes);
        crate::task::stage_at(Kind::Diligence, 1);
        let scanned = crate::auditx::scan_once(&eps, &g)?;
        crate::task::stage_at(Kind::Diligence, 2);
        let book = crate::readerx::read_scanned(&scanned, &who, &bytes)?;
        let mut r = crate::diligx::assemble(book, &scanned.fragment, &bytes, &work, window)?;
        r.from = found.supply.as_ref().map(|s| (s.level, s.place.clone()));
        r.files = found.supply.as_ref().and_then(|s| s.files);
        r.misses = found.misses;
        // The chain's current time (the latest among this chain's endpoints): grant badges need it; without
        // it they still show "no reading".
        r.now = crate::chainx::head_time(&eps, g.chain).ok().map(|(t, _, _)| t);
        Ok(Done::Diligence(Box::new(r)))
    }))
}

pub(super) fn save_snapshot(shell: &mut Shell, to: &str) -> Result<usize, crate::fault::Fault> {
    let to = crate::home::landing(to)?;
    let r = shell.diligence.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NotAudited, crate::lang::t(crate::lang::Key::Tail056).to_string())
    })?;
    crate::diligx::snapshot(r, &to)
}
