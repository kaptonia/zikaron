use super::*;

/// The open home's main network (chain id, registry), if configured. A read-only network may not duplicate it.
fn main_network(shell: &Shell) -> Option<(u64, Address)> {
    match (shell.settings.chain_id, shell.settings.registry) {
        (Some(c), Some(r)) => Some((c, r)),
        _ => None,
    }
}

fn registry_of(typed: &str) -> Result<Address, crate::fault::Fault> {
    Address::parse(typed).ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::AddressShape, typed.trim().to_string()))
}

/// Adds or changes a read-only network: parses the fields (`readnets::cells`) and writes the table without
/// contacting any chain. A changed network's last reading is dropped. Returns the number of networks.
pub fn save_read_network(
    shell: &mut Shell,
    was: Option<(u64, String)>,
    name: &str,
    chain: &str,
    registry: &str,
    from_block: &str,
    nodes: &str,
) -> Result<usize, crate::fault::Fault> {
    let net = crate::readnets::cells(name, chain, registry, from_block, nodes)?;
    let was = match was {
        Some((c, r)) => Some((c, registry_of(&r)?)),
        None => None,
    };
    let nets = crate::readnets::save(&crate::home::machine_dir()?, was, net, main_network(shell))?;
    shell.net_reads.retain(|(c, r, _)| Some((*c, *r)) != was && nets.iter().any(|n| n.is(*c, r)));
    let n = nets.len();
    shell.read_nets = Some(nets);
    Ok(n)
}

/// Removes a read-only network. Returns the number of networks left.
pub fn remove_read_network(shell: &mut Shell, chain: u64, registry: &str) -> Result<usize, crate::fault::Fault> {
    let r = registry_of(registry)?;
    let nets = crate::readnets::remove(&crate::home::machine_dir()?, chain, &r)?;
    shell.net_reads.retain(|(c, x, _)| !(*c == chain && *x == r));
    let n = nets.len();
    shell.read_nets = Some(nets);
    Ok(n)
}

/// Reads one read-only network once in the background (`Kind::ReadNet`): its nodes and the code at its
/// registry (`widex::gate`). The network is taken from the table as currently on disk.
pub fn read_read_network(shell: &mut Shell, chain: u64, registry: &str) -> Result<Spawned, crate::fault::Fault> {
    let r = registry_of(registry)?;
    // The on-disk table also becomes the shell's table; a reading is kept only for a row the shell still holds
    // with the same nodes, so both sides read one source.
    let nets = read_nets_now()?;
    // Drop the reading of any row whose nodes on disk differ from the nodes it was read with.
    let was = shell.read_nets.clone().unwrap_or_default();
    shell.net_reads.retain(|(c, x, _)| {
        let nodes_of = |t: &[crate::readnets::Net]| t.iter().find(|n| n.is(*c, x)).map(|n| n.nodes.clone());
        nodes_of(&was).is_some() && nodes_of(&was) == nodes_of(&nets)
    });
    shell.read_nets = Some(nets.clone());
    let net = nets
        .into_iter()
        .find(|n| n.is(chain, &r))
        .ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::SubjectMissing, format!("{chain} {}", r.hex())))?;
    Ok(shell.tasks.spawn(Kind::ReadNet, move || {
        crate::task::stage_at(Kind::ReadNet, 0);
        Ok(Done::NetRead { chain_id: net.chain_id, registry: net.registry, reading: crate::widex::gate(&net), nodes: net.nodes })
    }))
}
