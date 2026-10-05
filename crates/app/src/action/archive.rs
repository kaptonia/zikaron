use super::*;

/// Everything an export of a record bundle can be refused for without reading the chain: that it could be
/// recorded, a home, the folder the person gave and the holder it lands under. Asked before the exit gate
/// starts and again by the export where the gate landed; the judgment lives only here. Answers where the
/// bundle lands.
pub(super) fn mirror_plan(shell: &Shell, to: &str) -> Result<std::path::PathBuf, crate::fault::Fault> {
    // Ask whether it can be recorded first, then export. The requirement for a landed export includes recording
    // the place and time; in the reverse order, the whole bundle would be written first and then fail at
    // recording, leaving the person a bundle nobody remembers while the face said failure (on disk but not on
    // record is exactly the silent failure form).
    shell.may_save_settings()?;
    shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    // Holder: the current identity's current seat address in the register; without a register, the address of
    // the key available now.
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

/// `to` is the folder the person chose; the bundle lands where `mirror::bundle_in` assembles it for the
/// current identity and seat.
pub(super) fn export_mirror(shell: &mut Shell, to: &str, pass: &crate::exitgate::Pass) -> Result<(String, usize, usize, bool), crate::fault::Fault> {
    let target = mirror_plan(shell, to)?;
    // The exit gate passed in the background just before this runs (`gate_first`).
    let home = shell.home.as_ref().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    let made = crate::mirror::export(pass, home, &target, now_secs())?;
    // Recorded only once landed. What is recorded is this action's reading: where it landed and when.
    let record = crate::settings::MirrorRecord { path: made.root.display().to_string(), at: now_secs() };
    shell.commit_settings(|s| s.mirror = Some(record))?;
    // Measure again after exporting: the mirror slot cell is then computed in the background (no disk in the
    // frame); without remeasuring, that cell would keep showing the reading from before the export.
    if let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) {
        shell.tasks.spawn(Kind::Archive, move || measure(&root));
    }
    Ok((made.root.display().to_string(), made.entries, made.added, made.topped_up))
}

/// Relist after the vault changed. With one of the same kind in flight, record "dirty" and list again when it
/// lands (otherwise the in-flight pass returns the list from before the change, nobody lists again, and a
/// newly added item never appears on the face).
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

/// Reconcile once. Runs on a background thread.
pub(super) fn reconcile(root: &std::path::Path) -> Result<Done, crate::fault::Fault> {
    crate::task::stage_at(crate::task::Kind::Reconcile, 0);
    let home = crate::home::Home::open(root)?;
    let v = crate::auditx::offline(&home)?;
    Ok(Done::Reconciled { label: v.label, complete: v.complete, entries: v.entries })
}

/// Query the chain once. Runs on a background thread: network work in particular may never happen in the
/// frame.
pub(super) fn read_chain(eps: &[crate::chainx::Endpoint], who: &crate::key::Address, chain: Option<u64>) -> Result<Done, crate::fault::Fault> {
    crate::task::stage_at(crate::task::Kind::Chain, 0);
    // The basis chain's balance: the configured chain, or the first endpoint's when none is set.
    let Some(chain) = chain.or_else(|| eps.first().map(|e| e.chain)) else {
        return Err(crate::fault::Fault::known(crate::fault::Known::NoEndpoint, String::new()));
    };
    let (wei, reading) = crate::chainx::balance(eps, chain, who)?;
    // Having talked to a node, record the chain's current time (asked only when the balance answered, so an
    // unreachable node is not waited on again).
    let head_time = crate::chainx::head_time(eps, chain).ok().map(|x| x.0);
    Ok(Done::Chain {
        gas_wei: Some(wei),
        sources: reading.sources,
        single_source: reading.single_source,
        head_time,
    })
}

pub(super) fn set_endpoints(shell: &mut Shell, specs: &str) -> Result<usize, crate::fault::Fault> {
    let mut eps = Vec::new();
    for one in specs.split_whitespace() {
        let e = crate::chainx::Endpoint::parse(one).ok_or_else(|| {
            crate::fault::Fault::known(
                crate::fault::Known::SettingsShape,
                crate::lang::filln(crate::lang::Key::Tail008, &[&format!("{:?}", one)]),
            )
        })?;
        eps.push(e);
    }
    let specs: Vec<String> = eps.iter().map(|e| e.spec()).collect();
    // The person configured nodes themselves: this home's network no longer comes from a choice (that face
    // line goes dark).
    shell.commit_settings(|s| {
        s.endpoints = specs;
        s.network = None;
    })?;
    endpoints_changed(shell, eps);
    // The seat's nodes are set either way; the identity's record failing to take them is a trouble said by
    // name.
    if let Err(f) = remember_custom(shell) {
        shell.faults.push(f);
    }
    Ok(shell.endpoints.len())
}

/// After endpoints change, the shell's related cells are voided in one place (the person editing nodes and
/// taking a known deployment row go through here alike).
pub(super) fn endpoints_changed(shell: &mut Shell, eps: Vec<crate::chainx::Endpoint>) {
    shell.endpoints = eps;
    // The "chain read" time and the network sentence speak of the replaced nodes: void them and wait for the
    // new nodes to answer once.
    shell.chain_read_at = None;
    shell.status = None;
    // The last self-audit report is voided too (as with a source change): it came from the replaced nodes,
    // and the green lights and the "anchored" question (the third precondition of `queue_entry`) read it.
    // Keeping it would show the old nodes' words after the person changed nodes. Clearing it makes the next
    // frame audit again (`audit_stale` counts no report as due).
    shell.audit = None;
    shell.audit_asked = None;
    // New nodes answer the tail question afresh (a marked home checks again: `tail_if_due` below).
    shell.tail_asked = None;
    // The last pass's "anchored" set (the verdict cache) keeps only rows of the current chain (after a
    // network change, old chain rows do not pass for "last verified").
    let chain = shell.settings.chain_id;
    if let Some(r) = shell.remembered.as_mut() {
        r.rows.retain(|(_, (c, _))| Some(*c) == chain);
    }
    // The table's green lights were lit by that report: with the report voided, the table is reread too (the
    // same exit as audit landing).
    shell.stale_rows();
    tail_if_due(shell);
}

/// A home takes a network only through this function: a known row as [`adopt_deployment`]; one filled in by
/// hand as recorded (its chain, registry, start block and nodes), the home's network cell then "custom". Two
/// callers: a writer opening a home without a network whose identity has one (`open_home_at`), and the wizard's
/// network step choosing a row (`choose_network`, through [`adopt_deployment`]). It saves once and records
/// which choice it came from (the face line lights by it).
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

/// The open home is left without a network (the wizard chose "custom" for it): no chain, no registry, no
/// nodes, the start block back to zero, no record of a choice. Reading the chain and putting on chain then
/// refuse by the names of a home with no network, until it is filled in settings.
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

/// After a seat configures its network by hand, for an identity that chose "custom": once the seat has the
/// whole of it (chain, registry, nodes), it is recorded on that identity's row, so the identity's other seat
/// takes the same network. An identity that chose a row, a home no identity owns, or a network not yet whole
/// records nothing (the seat's own settings stay the seat's).
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
