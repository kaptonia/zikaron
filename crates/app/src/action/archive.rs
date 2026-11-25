use super::*;

pub(super) fn mirror_plan(shell: &Shell, to: &str) -> Result<std::path::PathBuf, crate::fault::Fault> {
    shell.may_save_settings()?;
    shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
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

pub(super) fn export_mirror(shell: &mut Shell, to: &str, pass: &crate::exitgate::Pass) -> Result<(String, usize, usize, bool), crate::fault::Fault> {
    let target = mirror_plan(shell, to)?;
    let home = shell.home.as_ref().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    let made = crate::mirror::export(pass, home, &target, now_secs())?;
    let record = crate::settings::MirrorRecord { path: made.root.display().to_string(), at: now_secs() };
    shell.commit_settings(|s| s.mirror = Some(record))?;
    if let Err(f) = crate::exports::record_plain(&made.root, crate::exports::Kind::Mirror) {
        shell.faults.push(f);
    }
    let _ = remeasure(shell);
    Ok((made.root.display().to_string(), made.entries, made.added, made.topped_up))
}

pub(super) fn relist_held(shell: &mut Shell) {
    shell.vault_changed = true;
    shell.held = None;
    shell.cards = None;
    shell.tasks.forget(Kind::Held);
    match list_held(shell) {
        Ok(Spawned::Started) => {}
        Ok(Spawned::InFlight) => shell.held_dirty = true,
        Err(f) => shell.faults.push(f),
    }
}

pub(super) fn reconcile(root: &std::path::Path) -> Result<Done, crate::fault::Fault> {
    crate::task::stage_at(crate::task::Kind::Reconcile, 0);
    let home = crate::home::Home::open(root)?;
    let v = crate::auditx::offline(&home)?;
    Ok(Done::Reconciled { label: v.label, complete: v.complete, entries: v.entries })
}

pub(super) fn read_chain(eps: &[crate::chainx::Endpoint], who: &crate::key::Address, chain: Option<u64>) -> Result<Done, crate::fault::Fault> {
    let _read = crate::chainx::OneRead::open();
    crate::task::stage_at(crate::task::Kind::Chain, 0);
    let Some(chain) = chain.or_else(|| eps.first().map(|e| e.chain)) else {
        return Err(crate::fault::Fault::known(crate::fault::Known::NoEndpoint, String::new()));
    };
    let (wei, reading) = crate::chainx::balance(eps, chain, who)?;
    let head_time = crate::chainx::head_time(eps, chain).ok().map(|x| x.0);
    Ok(Done::Chain {
        gas_wei: Some(wei),
        sources: reading.sources,
        single_source: reading.single_source,
        unanswered: reading.unanswered,
        differed: reading.differed,
        head_time,
    })
}

pub(super) fn set_log_asks(shell: &mut Shell, n: u64) -> Result<u64, crate::fault::Fault> {
    if !zikaron_anchor::action::asks_within(n) {
        return Err(crate::fault::Fault::known(crate::fault::Known::SettingsShape, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::LogAsksRange).to_string())));
    }
    shell.commit_settings(|s| s.log_asks = Some(n))?;
    zikaron_anchor::action::take_asks_cap(n);
    Ok(n)
}

pub(super) fn set_endpoints(shell: &mut Shell, specs: &str) -> Result<usize, crate::fault::Fault> {
    let mut eps = Vec::new();
    for one in specs.split_whitespace() {
        let e = crate::chainx::Endpoint::typed(one).map_err(|w| crate::fault::Fault::known(crate::fault::Known::SettingsShape, crate::lang::both_of(|| crate::chainx::said_of(&w))))?;
        eps.push(e);
    }
    let specs: Vec<String> = eps.iter().map(|e| e.spec()).collect();
    shell.commit_settings(|s| {
        s.endpoints = specs;
        s.network = None;
    })?;
    endpoints_changed(shell);
    if let Err(f) = remember_custom(shell) {
        shell.faults.push(f);
    }
    super::audit::check_basis_after_nodes(shell);
    Ok(shell.endpoints.len())
}

pub(super) fn endpoints_changed(shell: &mut Shell) {
    shell.endpoints = crate::chainx::nodes_of(&shell.settings);
    shell.tasks.nodes_moved();
    shell.chain_read_at = None;
    shell.status = None;
    shell.chain = None;
    shell.failed.remove(&crate::task::Kind::Chain);
    let served = shell.settings.chain_id.map_or(!shell.endpoints.is_empty(), |c| shell.endpoints.iter().any(|e| e.chain == c));
    if let (Some(who), true) = (shell.anchor, served) {
        let (eps, chain) = (shell.endpoints.clone(), shell.settings.chain_id);
        let _ = shell.tasks.spawn(crate::task::Kind::Chain, move || read_chain(&eps, &who, chain));
    }
    shell.audit = None;
    shell.audit_asked = None;
    shell.tail_asked = None;
    let chain = shell.settings.chain_id;
    if let Some(r) = shell.remembered.as_mut() {
        r.rows.retain(|(_, (c, _))| Some(*c) == chain);
    }
    shell.reread(crate::shell::Reread::Landed);
    tail_if_due(shell);
}

pub(super) fn adopt_network(shell: &mut Shell, n: crate::deploy::Network) -> Result<(), crate::fault::Fault> {
    shell.commit_settings(|s| network_into(s, n))?;
    endpoints_changed(shell);
    Ok(())
}

pub(super) fn network_into(s: &mut crate::settings::Settings, n: crate::deploy::Network) {
    match n {
        crate::deploy::Network::Known(d) => {
            s.chain_id = Some(d.chain_id);
            s.registry = Some(d.registry_address());
            s.from_block = d.from_block;
            s.endpoints = d.endpoint_specs().iter().filter_map(|x| crate::chainx::Endpoint::parse(x)).map(|e| e.spec()).collect();
            s.network = Some(d.name.to_string());
        }
        crate::deploy::Network::Custom(c) => {
            s.chain_id = Some(c.chain_id);
            s.registry = Some(c.registry);
            s.from_block = c.from_block;
            s.endpoints = c.endpoints.iter().filter_map(|x| crate::chainx::Endpoint::parse(x)).map(|e| e.spec()).collect();
            s.network = Some(crate::deploy::CUSTOM.to_string());
        }
    }
}

pub(super) fn chain_nodes(settings: &crate::settings::Settings, chain: u64, row: &crate::deploy::Deployment) -> Vec<String> {
    let own: Vec<String> = settings.endpoints.iter().filter_map(|x| crate::chainx::Endpoint::parse(x)).filter(|e| e.chain == chain).map(|e| e.spec()).collect();
    if !own.is_empty() {
        return own;
    }
    row.endpoint_specs().iter().filter_map(|x| crate::chainx::Endpoint::parse(x)).map(|e| e.spec()).collect()
}

pub(super) fn reroot_network(s: &mut crate::settings::Settings, row: &'static crate::deploy::Deployment, nodes: Vec<String>) {
    network_into(s, crate::deploy::Network::Known(row));
    s.endpoints = nodes;
}

pub(super) fn clear_network(shell: &mut Shell) -> Result<(), crate::fault::Fault> {
    shell.commit_settings(|s| {
        s.chain_id = None;
        s.registry = None;
        s.from_block = 0;
        s.endpoints.clear();
        s.network = None;
    })?;
    endpoints_changed(shell);
    Ok(())
}

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
        shell.seat_identities(Some(crate::register::view()?));
    }
    Ok(())
}
