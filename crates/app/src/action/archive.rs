use super::*;

/// Everything a record bundle export can be refused for without reading the chain: whether settings can be
/// saved, a home, the folder given and the holder it lands under. Checked before the exit gate starts and
/// again by the export after it passes; this is the only place that decides. Returns where the bundle lands.
pub(super) fn mirror_plan(shell: &Shell, to: &str) -> Result<std::path::PathBuf, crate::fault::Fault> {
    // Check that settings can be saved before exporting: a finished export records its place and time, and in
    // the other order a whole bundle could be written and then fail to be recorded, leaving an untracked
    // bundle on disk while the UI reported failure.
    shell.may_save_settings()?;
    shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    // Holder: the address of the current identity's current seat in the register; without a register, the
    // address of the key available now.
    let seat = shell.settings.role;
    let holder = crate::register::now_row_listed()
        .ok()
        .flatten()
        .and_then(|(row, _)| row.address(seat))
        .or(shell.anchor)
        .map(|a| a.hex())
        .unwrap_or_default();
    crate::mirror::bundle_in(&crate::home::landing(to)?, &holder, seat)
}

/// Exports the record bundle. `to` is the folder the person chose; the bundle lands where `mirror::bundle_in`
/// places it for the current identity and seat.
pub(super) fn export_mirror(shell: &mut Shell, to: &str, pass: &crate::exitgate::Pass) -> Result<(String, usize, usize, bool), crate::fault::Fault> {
    let target = mirror_plan(shell, to)?;
    // The exit gate passed in the background just before this runs (`gate_first`).
    let home = shell.home.as_ref().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    let made = crate::mirror::export(pass, home, &target, now_secs())?;
    // Record the export only once it has landed: where and when.
    let record = crate::settings::MirrorRecord { path: made.root.display().to_string(), at: now_secs() };
    shell.commit_settings(|s| s.mirror = Some(record))?;
    // Re-measure after exporting so the bundle status is recomputed in the background; otherwise it would keep
    // showing the state from before the export.
    if let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) {
        shell.tasks.spawn(Kind::Archive, move || measure(&root));
    }
    Ok((made.root.display().to_string(), made.entries, made.added, made.topped_up))
}

/// Relists the vault after it changed. If a listing is already in flight, mark it dirty and list again when it
/// lands; otherwise the in-flight listing would return the old contents and a new item would never appear.
pub(super) fn relist_held(shell: &mut Shell) {
    shell.held = None;
    shell.cards = None;
    shell.tasks.forget(Kind::Held);
    match list_held(shell) {
        Ok(Spawned::Started) => {}
        Ok(Spawned::InFlight) => shell.held_dirty = true,
        Err(f) => shell.faults.push(f),
    }
}

/// Reconciles once, on a background thread.
pub(super) fn reconcile(root: &std::path::Path) -> Result<Done, crate::fault::Fault> {
    crate::task::stage_at(crate::task::Kind::Reconcile, 0);
    let home = crate::home::Home::open(root)?;
    let v = crate::auditx::offline(&home)?;
    Ok(Done::Reconciled { label: v.label, complete: v.complete, entries: v.entries })
}

/// Queries the chain once, on a background thread (network work never runs on the UI thread).
pub(super) fn read_chain(eps: &[crate::chainx::Endpoint], who: &crate::key::Address, chain: Option<u64>) -> Result<Done, crate::fault::Fault> {
    crate::task::stage_at(crate::task::Kind::Chain, 0);
    // The basis chain's balance: the configured chain, or the first endpoint's when none is set.
    let Some(chain) = chain.or_else(|| eps.first().map(|e| e.chain)) else {
        return Err(crate::fault::Fault::known(crate::fault::Known::NoEndpoint, String::new()));
    };
    let (wei, reading) = crate::chainx::balance(eps, chain, who)?;
    // Record the chain's current time; asked only after the balance answered, so an unreachable node is not
    // waited on twice.
    let head_time = crate::chainx::head_time(eps, chain).ok().map(|x| x.0);
    Ok(Done::Chain {
        gas_wei: Some(wei),
        sources: reading.sources,
        single_source: reading.single_source,
        unanswered: reading.unanswered,
        head_time,
    })
}

pub(super) fn set_endpoints(shell: &mut Shell, specs: &str) -> Result<usize, crate::fault::Fault> {
    let mut eps = Vec::new();
    for one in specs.split_whitespace() {
        let e = crate::chainx::Endpoint::typed(one).map_err(|said| crate::fault::Fault::known(crate::fault::Known::SettingsShape, said))?;
        eps.push(e);
    }
    let specs: Vec<String> = eps.iter().map(|e| e.spec()).collect();
    // The person configured nodes by hand, so this home's network no longer comes from a preset choice.
    shell.commit_settings(|s| {
        s.endpoints = specs;
        s.network = None;
    })?;
    endpoints_changed(shell, eps);
    // The seat's nodes are set either way; failing to record them on the identity is reported by name.
    if let Err(f) = remember_custom(shell) {
        shell.faults.push(f);
    }
    // A custom basis saved earlier (or checked against other nodes) is now checked against these nodes, as
    // when the basis is saved.
    super::audit::check_basis_after_nodes(shell);
    Ok(shell.endpoints.len())
}

/// Resets the shell's endpoint-dependent state in one place, whether the person edited nodes or picked a
/// known deployment.
pub(super) fn endpoints_changed(shell: &mut Shell, eps: Vec<crate::chainx::Endpoint>) {
    shell.endpoints = eps;
    // The "chain read" time and the network status describe the replaced nodes: clear them until the new
    // nodes answer.
    shell.chain_read_at = None;
    shell.status = None;
    // Clear the last self-audit report too (as on a source change): it came from the replaced nodes, and the
    // status lights and the "already anchored" check in `queue_entry` read it. The next frame audits again
    // (`audit_stale` treats a missing report as due).
    shell.audit = None;
    shell.audit_asked = None;
    // New nodes answer the tail question afresh (`tail_if_due` below).
    shell.tail_asked = None;
    // The cached "anchored" set keeps only rows for the current chain, so after a network change old rows do
    // not count as last verified.
    let chain = shell.settings.chain_id;
    if let Some(r) = shell.remembered.as_mut() {
        r.rows.retain(|(_, (c, _))| Some(*c) == chain);
    }
    // The table's status lights came from that report, so reread the table too (as when an audit lands).
    shell.stale_rows();
    tail_if_due(shell);
}

/// The only way a home takes a network: a known deployment via [`adopt_deployment`], or a hand-filled one
/// stored as given (chain, registry, start block, nodes) with the home's network set to "custom". Called when
/// a writer opens a home without a network whose identity has one (`open_home_at`), and by the wizard's
/// network step (`choose_network`). Saves once and records which choice it came from.
pub(super) fn adopt_network(shell: &mut Shell, n: crate::deploy::Network) -> Result<(), crate::fault::Fault> {
    match n {
        crate::deploy::Network::Known(d) => adopt_deployment(shell, d),
        crate::deploy::Network::Custom(c) => {
            let c = c.clone();
            let eps: Vec<crate::chainx::Endpoint> = c.endpoints.iter().filter_map(|x| crate::chainx::Endpoint::parse(x)).collect();
            shell.commit_settings(|s| {
                s.chain_id = Some(c.chain_id);
                s.registry = Some(c.registry);
                s.from_block = c.from_block;
                s.endpoints = eps.iter().map(|e| e.spec()).collect();
                s.network = Some(crate::deploy::CUSTOM.to_string());
            })?;
            endpoints_changed(shell, eps);
            Ok(())
        }
    }
}

/// Leaves the open home without a network (the wizard chose "custom"): no chain, registry or nodes, start
/// block zero, no recorded choice. Chain reads and anchoring are refused as for a home with no network until
/// it is filled in settings.
pub(super) fn clear_network(shell: &mut Shell) -> Result<(), crate::fault::Fault> {
    shell.commit_settings(|s| {
        s.chain_id = None;
        s.registry = None;
        s.from_block = 0;
        s.endpoints.clear();
        s.network = None;
    })?;
    endpoints_changed(shell, Vec::new());
    Ok(())
}

/// For an identity that chose "custom": once a seat has configured the whole network by hand (chain,
/// registry, nodes), record it on the identity's row so the identity's other seat gets the same network.
/// Nothing is recorded for an identity that chose a known row, a home no identity owns, or an incomplete
/// network.
pub(super) fn remember_custom(shell: &mut Shell) -> Result<(), crate::fault::Fault> {
    let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) else { return Ok(()) };
    let s = &shell.settings;
    let (Some(chain_id), Some(registry)) = (s.chain_id, s.registry) else { return Ok(()) };
    if s.endpoints.is_empty() {
        return Ok(());
    }
    let c = crate::deploy::Custom { chain_id, registry, from_block: s.from_block, endpoints: s.endpoints.clone() };
    let changed = crate::register::change_listed(|reg| match crate::identity::owner_of(reg, &root) {
        Some((row, _)) => crate::identity::remember_custom(reg, &row.id, c),
        None => Ok(false),
    })?;
    if changed == Some(true) {
        shell.seat_identities(Some(crate::register::view(shell.settings.role)?));
    }
    Ok(())
}

pub(super) fn adopt_deployment(shell: &mut Shell, d: &'static crate::deploy::Deployment) -> Result<(), crate::fault::Fault> {
    let eps: Vec<crate::chainx::Endpoint> = d.endpoint_specs().iter().filter_map(|x| crate::chainx::Endpoint::parse(x)).collect();
    shell.commit_settings(|s| {
        s.chain_id = Some(d.chain_id);
        s.registry = Some(d.registry_address());
        s.from_block = d.from_block;
        s.endpoints = eps.iter().map(|e| e.spec()).collect();
        s.network = Some(d.name.to_string());
    })?;
    endpoints_changed(shell, eps);
    Ok(())
}
