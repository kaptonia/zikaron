use super::*;

/// The check page, run on a background thread with no privileges: it reads no key, writes no home, sends no
/// transaction and saves no settings.
///
/// Endpoints and basis come from the page; empty fields borrow the open home's (read only), so it works
/// without an identity. Reading files, scanning the chain and judging all happen in `checkx::run`; here the
/// input only passes a shape check before going to the background.
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
    // The shape check only asks whether the input is empty; reading files and decoding happen in the
    // background.
    if typed.trim().is_empty() {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::FieldMissing,
            crate::lang::t(crate::lang::Key::Tail065).to_string(),
        ));
    }
    let typed = typed.trim().to_string();
    let mut shelf = shelf_of(shell, ledgers);
    shelf.reads = read_nets_now()?;
    // Endpoints: the page's if filled; otherwise the home's configured ones (read-only).
    let mut eps: Vec<crate::chainx::Endpoint> = Vec::new();
    let mut bad: Vec<String> = Vec::new();
    // A refused line is reported as `Endpoint::typed` words it, never echoing the line itself, which may carry
    // a node's API key. One sentence per line.
    for line in endpoints.lines().map(str::trim).filter(|l| !l.is_empty()) {
        match crate::chainx::Endpoint::typed(line) {
            Ok(e) => eps.push(e),
            Err(said) => bad.push(said),
        }
    }
    if !bad.is_empty() {
        return Err(crate::fault::Fault::known(crate::fault::Known::SettingsShape, bad.join(" · ")));
    }
    // Every field typed on the page is shape-checked before anything is borrowed or reported missing: an
    // unreadable registry or start block is refused by name here, never hidden behind "no registry
    // configured". Missing fields are reported later, where the check reads them.
    let registry_typed = registry.trim();
    let typed_registry = match registry_typed {
        "" => None,
        r => Some(Address::parse(r).ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::AddressShape, r.to_string()))?),
    };
    let typed_from: Option<u64> = match from_block.trim() {
        "" => None,
        f => Some(f.parse().map_err(|_| crate::fault::Fault::known(crate::fault::Known::SettingsShape, crate::lang::filln(crate::lang::Key::Tail025, &[&format!("{f:?}")])))?),
    };
    if eps.is_empty() {
        eps = shell.endpoints.clone();
    }
    // Compute the main chain id once, shared by the basis and chain time: the single endpoint's chain; with
    // several chains, the home's configured chain if it is among them; otherwise none. Chain time needs only a
    // chain id, so it must not depend on the basis, which also needs a registry.
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
    // Basis: the chain id from above, registry and start block from the page, or the home's when not filled.
    let ground: Result<crate::auditx::Ground, crate::fault::Fault> = (|| {
        let reg = match typed_registry {
            Some(r) => r,
            None => shell.settings.registry.ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoRegistry, String::new()))?,
        };
        let from: u64 = match (typed_from, typed_registry) {
            (Some(f), _) => f,
            (None, None) => shell.settings.from_block,
            (None, Some(_)) => 0,
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

/// The record place for a page that reads a single place (reader, diligence). More than one non-empty line
/// (split at `\n`, `\r` or both) is refused by name rather than silently reading only the first; the error
/// carries the line count.
pub fn one_place(dir: &str) -> Result<&str, crate::fault::Fault> {
    let lines = dir.split(['\n', '\r']).filter(|l| !l.trim().is_empty()).count();
    if lines > 1 {
        return Err(crate::fault::Fault::known(crate::fault::Known::PlaceOneLine, lines.to_string()));
    }
    Ok(dir.trim())
}

/// The places the check page and vault re-check take material from (`supplyx::Shelf`): every seat's home in
/// the local register, the vault's kept grant file and recorded upstream locations, and the places the person
/// gives (one line per hop). Only paths and addresses; disk reads happen in the background.
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

/// The read-only networks currently in the machine directory, for paths that read someone else's material.
/// No machine directory means no table; an unreadable table refuses the action by name.
pub fn read_nets_now() -> Result<Vec<crate::readnets::Net>, crate::fault::Fault> {
    match crate::home::machine_dir() {
        Ok(m) => crate::readnets::read(&m),
        Err(_) => Ok(Vec::new()),
    }
}
