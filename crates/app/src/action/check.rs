use super::*;

/// Check. Runs on a background thread; zero permissions: reads no key, writes no home, sends no transaction,
/// saves no settings.
///
/// Endpoints and basis are filled in on the page; when the page is empty it borrows the open home's (reading,
/// never changing it), so it works without an identity. Reading files, scanning the chain and judging are all
/// in `checkx::run`; here the input only passes a shape gate before going to the background.
pub(super) fn check_payload(
    shell: &mut Shell,
    typed: &str,
    ledgers: &str,
    endpoints: &str,
    registry: &str,
    from_block: &str,
    now: &str,
    (file, terms): (String, String),
) -> Result<Spawned, crate::fault::Fault> {
    let injected = crate::checkx::now_of(now)?;
    // The input shape gate only asks "empty or not"; reading files and decoding happen in the background (no
    // disk in the frame).
    if typed.trim().is_empty() {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::FieldMissing,
            crate::lang::t(crate::lang::Key::Tail065).to_string(),
        ));
    }
    let typed = typed.trim().to_string();
    let mut shelf = shelf_of(shell, ledgers);
    shelf.reads = read_nets_now()?;
    // Endpoints: the page's when filled; otherwise borrow the home's configured ones (read-only).
    let mut eps: Vec<crate::chainx::Endpoint> = Vec::new();
    let mut bad: Vec<String> = Vec::new();
    for line in endpoints.lines().map(str::trim).filter(|l| !l.is_empty()) {
        match crate::chainx::Endpoint::parse(line) {
            Some(e) => eps.push(e),
            None => bad.push(line.to_string()),
        }
    }
    if !bad.is_empty() {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::SettingsShape,
            crate::lang::filln(crate::lang::Key::Tail066, &[&(bad.join(" ")).to_string()]),
        ));
    }
    if eps.is_empty() {
        eps = shell.endpoints.clone();
    }
    // Compute the main chain id once, shared by basis and chain time. With a single endpoint it is that one;
    // when endpoints span several chains the home's configured main chain id wins (it must be among the
    // endpoints); with neither there is none. Computing it only on the basis branch would make chain time
    // absent whenever the registry is not configured (basis cannot be built), while chain time needs only a
    // chain id.
    let chain: Option<u64> = {
        let mut chains: Vec<u64> = eps.iter().map(|e| e.chain).collect();
        chains.sort_unstable();
        chains.dedup();
        match (chains.len(), shell.settings.chain_id) {
            (1, _) => Some(chains[0]),
            (0, c) => c,
            (_, Some(c)) if chains.contains(&c) => Some(c),
            _ => None,
        }
    };
    // Basis: the chain id from above, registry and start block from the page; when not filled, borrow the
    // home's.
    let ground: Result<crate::auditx::Ground, crate::fault::Fault> = (|| {
        let registry_typed = registry.trim();
        let reg = if registry_typed.is_empty() {
            shell.settings.registry.ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoRegistry, String::new()))?
        } else {
            Address::parse(registry_typed).ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::AddressShape, registry_typed.to_string()))?
        };
        let from: u64 = if from_block.trim().is_empty() {
            if registry_typed.is_empty() { shell.settings.from_block } else { 0 }
        } else {
            from_block.trim().parse().map_err(|_| {
                crate::fault::Fault::known(
                    crate::fault::Known::SettingsShape,
                    crate::lang::filln(crate::lang::Key::Tail025, &[&format!("{:?}", from_block.trim())]),
                )
            })?
        };
        let chain = chain.ok_or_else(|| {
            crate::fault::Fault::known(
                crate::fault::Known::NoChainId,
                if eps.is_empty() { String::new() } else { crate::lang::t(crate::lang::Key::Tail067).to_string() },
            )
        })?;
        Ok(crate::auditx::Ground { chain, registry: reg, from_block: from, to_block: from, senders: Vec::new() })
    })();
    shell.checked = None;
    Ok(shell.tasks.spawn(Kind::Check, move || {
        crate::task::stage_at(Kind::Check, 0);
        let hops = crate::checkx::hops_of(&typed)?;
        let checked = crate::checkx::run(hops, shelf, eps, ground, injected, chain);
        Ok(Done::Checked(Box::new(crate::checkx::with_sides(checked, &file, &terms))))
    }))
}

/// The places the check page and vault re-check take material from (`supplyx`'s `Shelf`), gathered here in
/// one place: every seat's home in the local register, the vault's kept grant file room and the recorded
/// upstream locations, and the places the person points to (one line per hop). Only paths and addresses; disk
/// reads happen in the background.
pub fn shelf_of(shell: &Shell, manual: &str) -> crate::supplyx::Shelf {
    let mut homes: Vec<(String, std::path::PathBuf)> = Vec::new();
    if let Some(reg) = shell.identities.as_ref() {
        for row in &reg.rows {
            for seat in row.seats() {
                if let (Some(a), Some(h)) = (row.address(seat), row.home(seat)) {
                    homes.push((a.hex(), h));
                }
            }
        }
    }
    crate::supplyx::Shelf {
        homes,
        kept: shell.home.as_ref().map(|h| h.dir(crate::home::Slot::GrantsHeld).join(crate::grantfilex::KEPT)),
        upstreams: shell.settings.upstreams.clone(),
        manual: manual.lines().map(|l| Some(l.trim().to_string()).filter(|x| !x.is_empty())).collect(),
        carried: None,
        pointer: None,
        reads: Vec::new(),
    }
}

/// The read-only networks as the machine directory holds them now, for a path that reads someone else's
/// material. A machine with no machine directory has no table; a table that cannot be read refuses the action
/// by name.
pub fn read_nets_now() -> Result<Vec<crate::readnets::Net>, crate::fault::Fault> {
    match crate::home::machine_dir() {
        Ok(m) => crate::readnets::read(&m),
        Err(_) => Ok(Vec::new()),
    }
}
