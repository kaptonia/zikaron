use super::*;

/// This desk's basis now (law §9.4). With the three cells incomplete it says by name what is not configured,
/// never asking the chain with half a basis.
pub(super) fn ground(shell: &Shell) -> Result<crate::auditx::Ground, crate::fault::Fault> {
    let chain = shell.settings.chain_id.ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoChainId, String::new())
    })?;
    let registry = shell.settings.registry.ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoRegistry, String::new())
    })?;
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let pile = home
        .ledger()?
        .pile()?;
    Ok(crate::auditx::Ground {
        chain,
        registry,
        from_block: shell.settings.from_block,
        // Scan to the latest: §9.4 needs a toBlock, and "the latest now" is answered by the endpoints.
        // The background pass asks it (see `run_audit`); here a placeholder lower bound is set.
        to_block: shell.settings.from_block,
        senders: crate::auditx::senders_of(&pile.items),
    })
}

pub(super) fn start_audit(shell: &mut Shell) -> Result<Spawned, crate::fault::Fault> {
    let g = ground(shell)?;
    let eps = shell.endpoints.clone();
    if eps.is_empty() {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::NoEndpoint,
            crate::lang::t(crate::lang::Key::Tail021).to_string(),
        ));
    }
    let root = shell
        .home
        .as_ref()
        .map(|h| h.root().to_path_buf())
        .ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    // Record the mark only for a pass that really started. The mark says "this version of the ledger was
    // asked"; recording it without starting (the previous pass still running) would vouch for an old report:
    // the running pass started with an older ledger, and after it lands the lights would not refresh.
    let spawned = shell.tasks.spawn(Kind::Audit, move || run_audit(&root, &eps, g));
    if matches!(spawned, Spawned::Started) {
        shell.audit_asked = Some(shell.book_mark);
    }
    Ok(spawned)
}

/// Run one self-audit. Runs on a background thread.
pub(super) fn run_audit(
    root: &std::path::Path,
    eps: &[crate::chainx::Endpoint],
    mut g: crate::auditx::Ground,
) -> Result<Done, crate::fault::Fault> {
    // `toBlock` is asked of the chain now: a fixed number would set the same height for every chain.
    // The basis chain's height, smallest over its endpoints (`head_block`): each endpoint reports its own,
    // a block or two apart, and every endpoint has reached the smallest, so the scan up to it still asks for
    // agreement. Asking every endpoint for one height that must match turned "one node is a block ahead"
    // into a disagreement.
    let (n, _) = crate::chainx::head_block(eps, g.chain)?;
    g.to_block = n.max(g.from_block);
    let home = crate::home::Home::open(root)?;
    let v = crate::auditx::online(&home, eps, &g)?;
    Ok(Done::Audited {
        broken: v.broken(),
        label: v.label,
        complete: v.complete,
        entries: v.entries,
        report: v.report,
        unanswered: v.unanswered,
        asked: v.asked,
        single_source: v.single_source,
        fragment: v.fragment,
    })
}

pub(super) fn set_basis(
    shell: &mut Shell,
    chain: &str,
    registry: &str,
    from_block: &str,
) -> Result<u64, crate::fault::Fault> {
    let c: u64 = chain.trim().parse().map_err(|_| {
        crate::fault::Fault::known(
            crate::fault::Known::SettingsShape,
            crate::lang::filln(crate::lang::Key::Tail024, &[&format!("{:?}", chain.trim())]),
        )
    })?;
    let r = Address::parse(registry).ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::AddressShape, registry.trim().to_string())
    })?;
    let f: u64 = from_block.trim().parse().map_err(|_| {
        crate::fault::Fault::known(
            crate::fault::Known::SettingsShape,
            crate::lang::filln(crate::lang::Key::Tail025, &[&format!("{:?}", from_block.trim())]),
        )
    })?;
    shell.commit_settings(|s| {
        s.chain_id = Some(c);
        s.registry = Some(r);
        s.from_block = f;
        s.network = None;
    })?;
    if let Err(f) = super::archive::remember_custom(shell) {
        shell.faults.push(f);
    }
    // A new basis answers the tail question afresh.
    shell.tail_asked = None;
    tail_if_due(shell);
    Ok(c)
}
