use super::*;

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
    if typed.trim().is_empty() {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::FieldMissing,
            crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail065).to_string()),
        ));
    }
    let typed = typed.trim().to_string();
    let mut shelf = shelf_of(shell, ledgers);
    shelf.reads = read_nets_now()?;
    let mut eps: Vec<crate::chainx::Endpoint> = Vec::new();
    let mut bad: Vec<String> = Vec::new();
    for line in endpoints.lines().map(str::trim).filter(|l| !l.is_empty()) {
        match crate::chainx::typed(line) {
            Ok(e) => eps.push(e),
            Err(said) => bad.push(said),
        }
    }
    if !bad.is_empty() {
        return Err(crate::fault::Fault::known(crate::fault::Known::SettingsShape, bad.join(" · ")));
    }
    let registry_typed = registry.trim();
    let typed_registry = match registry_typed {
        "" => None,
        r => Some(Address::parse(r).ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::AddressShape, r.to_string()))?),
    };
    let typed_from: Option<u64> = match from_block.trim() {
        "" => None,
        f => Some(f.parse().map_err(|_| crate::fault::Fault::known(crate::fault::Known::SettingsShape, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail025, &[&format!("{f:?}")]))))?),
    };
    if eps.is_empty() {
        eps = shell.endpoints.clone();
    }
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
                crate::lang::both_of(|| if eps.is_empty() { String::new() } else { crate::lang::t(crate::lang::Key::Tail067).to_string() }),
            )
        })?;
        Ok(crate::auditx::Ground { chain, registry: reg, from_block: from, to_block: from, senders: Vec::new(), earlier: Vec::new() })
    })();
    shell.checked = None;
    shell.check_asked = Some(CheckAsk { typed: typed.clone(), shelf: shelf.clone(), eps: eps.clone(), ground: ground.clone(), injected, chain });
    Ok(shell.tasks.spawn(Kind::Check, move || {
        crate::task::stage_at(Kind::Check, 0);
        let hops = crate::checkx::hops_of(&typed)?;
        let checked = crate::checkx::run(hops, shelf, eps, ground, injected, chain);
        Ok(Done::Checked(Box::new(crate::checkx::with_sides(checked, &file, &terms))))
    }))
}

#[derive(Clone, Debug)]
pub struct CheckAsk {
    pub typed: String,
    pub shelf: crate::supplyx::Shelf,
    pub eps: Vec<crate::chainx::Endpoint>,
    pub ground: Result<crate::auditx::Ground, crate::fault::Fault>,
    pub injected: Option<u64>,
    pub chain: Option<u64>,
}

pub(super) fn audit_check(shell: &mut Shell) -> Result<(), crate::fault::Fault> {
    let (Some(x), Some(asked)) = (shell.checked.as_ref(), shell.check_asked.clone()) else {
        return Err(crate::fault::Fault::known(crate::fault::Known::FieldMissing, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail065).to_string())));
    };
    let work = move || -> Result<crate::checkx::Audit, crate::fault::Fault> {
        let hops = crate::checkx::hops_of(&asked.typed)?;
        let x = crate::checkx::run_whole(hops, asked.shelf, asked.eps, asked.ground, asked.injected, asked.chain);
        match (&x.basis, &x.basis_why) {
            (Ok(_), _) => Ok(crate::checkx::Audit::of(&x)),
            (Err(_), Some(f)) => Err(f.clone()),
            (Err(said), None) => Err(crate::fault::Fault::unknown(said.clone())),
        }
    };
    shell.audits.ask(x.audit_key, crate::sendx::behind::Job { keeper: Kind::keeper(), work: Box::new(work) });
    behind_turn(shell);
    Ok(())
}

pub fn behind_turn(shell: &mut Shell) {
    if shell.tasks.in_flight(Kind::Behind) || !shell.audits.waiting() {
        return;
    }
    let audits = shell.audits.clone();
    let _ = shell.tasks.spawn(Kind::Behind, move || Ok(Done::Behind { taken: audits.run_each(|| {
        crate::task::land_part(Done::Behind { taken: 0 });
    }) }));
}

pub fn one_place(dir: &str) -> Result<&str, crate::fault::Fault> {
    let lines = dir.split(['\n', '\r']).filter(|l| !l.trim().is_empty()).count();
    if lines > 1 {
        return Err(crate::fault::Fault::known(crate::fault::Known::PlaceOneLine, lines.to_string()));
    }
    Ok(dir.trim())
}

pub fn shelf_of(shell: &Shell, manual: &str) -> crate::supplyx::Shelf {
    let mut homes: Vec<(String, std::path::PathBuf)> = Vec::new();
    if let Some(reg) = shell.identities.as_ref() {
        for row in &reg.rows {
            for seat in row.seats() {
                if let (Some(a), Some(h)) = (row.address(seat), row.home(seat)) {
                    homes.push((a.hex(), h.clone()));
                    if let Some(o) = row.office(seat) {
                        for k in std::iter::once(o.current).chain(o.former.iter().copied()).map(Address).filter(|k| *k != a) {
                            homes.push((k.hex(), h.clone()));
                        }
                    }
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

pub fn read_nets_now() -> Result<Vec<crate::readnets::Net>, crate::fault::Fault> {
    match crate::home::machine_dir() {
        Ok(m) => crate::readnets::read(&m),
        Err(_) => Ok(Vec::new()),
    }
}
