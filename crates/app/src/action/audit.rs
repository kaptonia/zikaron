use super::*;

/// The configured audit basis (chain, registry, start block, senders). If a setting is missing it is refused
/// by name, never querying the chain with a partial basis.
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
        // Placeholder: the background pass raises this to the chain's latest block (see `run_audit`).
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
    // Record the mark only when a pass actually started. The mark says this version of the ledger was audited;
    // setting it while an older pass is still running would vouch for that pass's stale report, and the lights
    // would not refresh after it lands.
    let spawned = shell.tasks.spawn(Kind::Audit, move || run_audit(&root, &eps, g));
    if matches!(spawned, Spawned::Started) {
        shell.audit_asked = Some(shell.book_mark);
    }
    Ok(spawned)
}

/// Runs one self-audit on a background thread.
pub(super) fn run_audit(
    root: &std::path::Path,
    eps: &[crate::chainx::Endpoint],
    mut g: crate::auditx::Ground,
) -> Result<Done, crate::fault::Fault> {
    // `to_block` is read from the chain now (a fixed number would be wrong for some chains): the lowest height
    // across the chain's endpoints (`head_block`). Endpoints may be a block or two apart and all have reached
    // the lowest, so the scan still requires agreement without treating a node one block ahead as a
    // disagreement.
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
) -> Result<Basis, crate::fault::Fault> {
    // Parse numbers the settings can store (`whole_within_ceiling`); anything past the ceiling is refused
    // before any node is asked.
    let c: u64 = crate::fault::whole_within_ceiling(chain, crate::lang::Key::Tail024)?;
    let r = Address::parse(registry).ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::AddressShape, registry.trim().to_string())
    })?;
    let f: u64 = crate::fault::whole_within_ceiling(from_block, crate::lang::Key::Tail025)?;
    // A chain and registry from the built-in table (`deploy::KNOWN`) are already pinned; save at once.
    if crate::deploy::KNOWN.iter().any(|d| d.chain_id == c && d.registry_address() == r) {
        write_basis(shell, c, r, f)?;
        shell.basis_read = None;
        return Ok(Basis::Saved(c));
    }
    // Any other registry is checked on save, not on read: on every node of this home for this chain that
    // answers, the code at that address must be the pinned registry build (`widex::gate_said`, the same gate
    // as for read-only networks; see [`basis_read`]). With no node for this chain yet there is nothing to ask:
    // the basis is saved as typed and shows "not checked" (people often fill the basis before the nodes), and
    // the same check runs when nodes for this chain are saved ([`check_basis_after_nodes`]).
    // The read-only network's node list keeps its written form (it is the same kind of list the machine keeps).
    let nodes: Vec<String> = shell.endpoints.iter().filter(|e| e.chain == c).map(|e| e.url.for_transport().to_string()).collect();
    if nodes.is_empty() {
        write_basis(shell, c, r, f)?;
        shell.basis_read = Some((c, r, None));
        return Ok(Basis::Saved(c));
    }
    Ok(Basis::Checking(spawn_check(shell, c, r, f, nodes, false)))
}

/// The single check of a custom main network, as a `Kind::Basis` task. It runs when the basis is saved with
/// nodes present, and when nodes are saved for a basis written unchecked.
fn spawn_check(shell: &mut Shell, c: u64, r: Address, f: u64, nodes: Vec<String>, after_nodes: bool) -> Spawned {
    let asked = basis_stamp(shell, c);
    let net = crate::readnets::Net { chain_id: c, registry: r, from_block: f, nodes, name: None };
    shell.tasks.spawn(Kind::Basis, move || {
        let (reading, said) = crate::widex::gate_said(&net);
        Ok(Done::BasisRead { chain: c, registry: r, from_block: f, reading, said, after_nodes, asked })
    })
}

/// After nodes are saved: if this home's main network is custom (not a built-in row) and the nodes cover its
/// chain, check it again on them, as when the basis is saved. The basis itself is never changed here (saving
/// nodes never removes the person's chain settings); the reading is shown beside it, and a registry that is
/// not the pinned build is reported ([`basis_read`]). If such a check is already in flight, the reading is
/// left as it is.
pub(super) fn check_basis_after_nodes(shell: &mut Shell) {
    let (Some(c), Some(r)) = (shell.settings.chain_id, shell.settings.registry) else { return };
    if crate::deploy::KNOWN.iter().any(|d| d.chain_id == c && d.registry_address() == r) {
        return;
    }
    // The read-only network's node list keeps its written form (it is the same kind of list the machine keeps).
    let nodes: Vec<String> = shell.endpoints.iter().filter(|e| e.chain == c).map(|e| e.url.for_transport().to_string()).collect();
    if nodes.is_empty() {
        return;
    }
    let f = shell.settings.from_block;
    if let Spawned::Started = spawn_check(shell, c, r, f, nodes, true) {
        shell.basis_read = Some((c, r, None));
    }
}

/// What a main-network check applies to, taken when it is asked and again when it lands: the main network as
/// stored in settings (chain, registry, start block, chosen row) and the nodes for the checked chain. Writing
/// any other setting meanwhile (language, audit interval) leaves the reading valid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BasisStamp {
    pub cell: u64,
    pub nodes: u64,
}

/// The stamp of what a check of chain `c` applies to now.
pub fn basis_stamp(shell: &Shell, c: u64) -> BasisStamp {
    use std::hash::{Hash, Hasher};
    let s = &shell.settings;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (s.chain_id, s.registry.map(|r| r.0), s.from_block, s.network.clone()).hash(&mut h);
    let cell = h.finish();
    let mut nodes: Vec<&str> = shell.endpoints.iter().filter(|e| e.chain == c).map(|e| e.url.for_transport()).collect();
    nodes.sort_unstable();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    nodes.hash(&mut h);
    BasisStamp { cell, nodes: h.finish() }
}

/// The immediate outcome of saving the main network: saved (a built-in deployment, or a chain with no node
/// yet, written unchecked), or a check started.
pub(super) enum Basis {
    Saved(u64),
    Checking(Spawned),
}

/// Handles a finished main-network check.
///
/// On a basis save, the basis is written only if the registry read as the pinned build (on one node, or
/// several agreeing); a different fingerprint, or no node answering, is reported and nothing is written. After
/// a node save (`after_nodes`), nothing is written or removed: the reading is shown and a fingerprint that is
/// not the pinned build is reported.
///
/// A reading applies only to what it was asked for (`asked`, [`BasisStamp`]). If the main network changed
/// since (another network chosen, the basis saved again), the reading is dropped, since the later choice wins.
/// If only the chain's nodes changed, the same check reruns on the current nodes, so a save is not lost and no
/// tick shows for nodes never asked. Other settings written meanwhile change nothing.
///
/// Returns whether the reading was taken. The reading stays beside the basis; "not checked" is never shown as
/// checked.
#[allow(clippy::too_many_arguments)]
pub fn basis_read(shell: &mut Shell, c: u64, r: Address, f: u64, reading: crate::widex::Reading, said: Option<crate::fault::Fault>, after_nodes: bool, asked: BasisStamp) -> bool {
    let now = basis_stamp(shell, c);
    if asked != now {
        if shell.basis_read.is_some_and(|(bc, br, _)| (bc, br) == (c, r)) {
            shell.basis_read = None;
        }
        if after_nodes || asked.cell != now.cell {
            check_basis_after_nodes(shell);
        } else {
            // A save whose chain's nodes changed meanwhile: save again as typed, on the current nodes.
            match set_basis(shell, &c.to_string(), &r.hex(), &f.to_string()) {
                Ok(_) => {}
                Err(e) => shell.faults.push(e),
            }
        }
        return false;
    }
    if after_nodes {
        if (shell.settings.chain_id, shell.settings.registry) != (Some(c), Some(r)) {
            return false;
        }
        shell.basis_read = Some((c, r, Some(reading)));
        if reading == crate::widex::Reading::Fingerprint {
            if let Some(f) = said {
                shell.faults.push(f);
            }
        }
        return true;
    }
    shell.basis_read = Some((c, r, Some(reading)));
    if !reading.admits() {
        if let Some(f) = said {
            shell.faults.push(f);
        }
        return true;
    }
    if let Err(f) = write_basis(shell, c, r, f) {
        shell.faults.push(f);
    }
    true
}

/// Writes the main network's basis: chain, registry, start block (a value the settings cannot store is refused
/// by name and nothing is written). The identity's record follows; a failure there is reported as a fault.
fn write_basis(shell: &mut Shell, c: u64, r: Address, f: u64) -> Result<(), crate::fault::Fault> {
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
    Ok(())
}
