use super::*;

/// The configured basis (chain, registry, start block), without consulting the local ledger. Diligence reads
/// someone else's ledger with senders from that ledger's own lineage (`readerx::basis_for`), so the local
/// ledger need not have a genesis. `to_block` starts at the lower bound and the background pass raises it to
/// the chain head (`to_head`): a fixed height would be wrong for some chains, and a scan that ends at the
/// start block finds nothing.
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

/// Moves the basis's upper bound to the chain's latest block (the lowest across endpoints).
pub(super) fn to_head(
    eps: &[crate::chainx::Endpoint],
    mut g: crate::auditx::Ground,
) -> Result<crate::auditx::Ground, crate::fault::Fault> {
    let (head, _) = crate::chainx::head_block(eps, g.chain)?;
    g.to_block = head.max(g.from_block);
    Ok(g)
}

/// Runs one diligence pass on a background thread.
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
    // Ledger bytes are looked up at four levels (`supplyx::find_book`, as on the reader and check pages): this
    // machine, the vault, a record bundle, a publish address. The "record content" field gives the place for
    // the last two; the first two are tried even when it is empty. If none supplies bytes the result is "not
    // obtained", naming each level that failed.
    let shelf = shelf_of(shell, dir);
    let nets = read_nets_now()?;
    shell.diligence = None;
    Ok(shell.tasks.spawn(Kind::Diligence, move || {
        let found = crate::supplyx::find_book(&shelf, &who.hex());
        let bytes = found.supply.as_ref().map(|s| s.items.clone()).unwrap_or_default();
        // Scan the fragment once, so the label, anchor count and quantities all describe the same pass.
        crate::task::stage_at(Kind::Diligence, 0);
        let (g, scanned, missed) = if nets.is_empty() {
            let g = crate::readerx::basis_for(&to_head(&eps, g)?, &who, &bytes);
            crate::task::stage_at(Kind::Diligence, 1);
            let scanned = crate::auditx::scan_once(&eps, &g)?;
            (g, scanned, Vec::new())
        } else {
            // Across networks: each chain's head is fetched along with its window.
            crate::task::stage_at(Kind::Diligence, 1);
            let (scanned, missed) = crate::readerx::scan_wide(&eps, &g, &who, &bytes, &nets)?;
            (g, scanned, missed)
        };
        crate::task::stage_at(Kind::Diligence, 2);
        let mut book = crate::readerx::read_scanned(&scanned, &who, &bytes)?;
        crate::readerx::unread_where_missed(&mut book, &missed);
        let mut r = crate::diligx::assemble(book, &scanned.fragment, &bytes, &work, window)?;
        r.from = found.supply.as_ref().map(|s| (s.level, s.place.clone()));
        r.files = found.supply.as_ref().and_then(|s| s.files);
        r.misses = found.misses;
        r.missed = missed;
        // The chain's current time (the latest across its endpoints), used by grant badges; without it they
        // show "no reading".
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
