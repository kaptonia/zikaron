use super::*;

pub(super) fn ground(shell: &Shell) -> Result<crate::auditx::Ground, crate::fault::Fault> {
    let chain = shell.settings.chain_id.ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoChainId, String::new())
    })?;
    let registry = shell.settings.registry.ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoRegistry, String::new())
    })?;
    shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    Ok(crate::auditx::Ground {
        chain,
        registry,
        from_block: shell.settings.from_block,
        to_block: shell.settings.from_block,
        senders: Vec::new(),
        earlier: shell
            .basis_chains()
            .into_iter()
            .skip(1)
            .map(|(chain, registry, from_block)| crate::auditx::Window { chain, registry, from_block, to_block: from_block })
            .collect(),
    })
}

pub(super) fn ground_read(shell: &Shell) -> Result<crate::auditx::Ground, crate::fault::Fault> {
    let g = ground(shell)?;
    let pile = shell.home.as_ref().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?.ledger()?.pile()?;
    Ok(with_senders(g, &pile.items))
}

fn with_senders(mut g: crate::auditx::Ground, items: &[Vec<u8>]) -> crate::auditx::Ground {
    g.senders = crate::auditx::senders_of(items);
    g
}

pub(super) fn start_audit(shell: &mut Shell) -> Result<Spawned, crate::fault::Fault> {
    let g = ground(shell)?;
    let eps = shell.endpoints.clone();
    if eps.is_empty() {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::NoEndpoint,
            crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail021).to_string()),
        ));
    }
    let root = shell
        .home
        .as_ref()
        .map(|h| h.root().to_path_buf())
        .ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    let spawned = shell.tasks.spawn(Kind::Audit, move || run_audit(&root, &eps, g));
    if matches!(spawned, Spawned::Started) {
        shell.audit_asked = Some(shell.book_mark);
    }
    Ok(spawned)
}

pub(super) fn run_audit(
    root: &std::path::Path,
    eps: &[crate::chainx::Endpoint],
    mut g: crate::auditx::Ground,
) -> Result<Done, crate::fault::Fault> {
    let (n, _) = crate::chainx::head_block(eps, g.chain)?;
    g.to_block = n.max(g.from_block);
    let mut unread: Vec<String> = Vec::new();
    g.earlier.retain_mut(|w| match crate::chainx::head_block(eps, w.chain) {
        Ok((h, _)) => {
            w.to_block = h.max(w.from_block);
            true
        }
        Err(f) => {
            unread.push(format!("chainId={} {}", w.chain, f.evidence()));
            false
        }
    });
    let home = crate::home::Home::open(root)?;
    let pile = home.ledger()?.pile()?;
    if g.senders.is_empty() {
        g = with_senders(g, &pile.items);
    }
    let mut v = crate::auditx::online(&home, eps, &g)?;
    v.unanswered.extend(unread);
    crate::sendx::keep_noticed(eps, &pile.items);
    Ok(Done::Audited {
        broken: v.broken(),
        label: v.label,
        complete: v.complete,
        entries: v.entries,
        report: v.report,
        unanswered: v.unanswered,
        unproven: v.unproven,
        asked: v.asked,
        single_source: v.single_source,
        fragment: v.fragment,
        recounted: false,
    })
}

pub fn recount(shell: &mut Shell) -> bool {
    if !shell.audit_stale() {
        return false;
    }
    let Some(held) = shell.audit.clone() else { return false };
    if held.nodes != shell.endpoints || held.fragment.member("anchors").is_none() {
        return false;
    }
    let Ok(g) = ground(shell) else { return false };
    let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) else { return false };
    let eps = shell.endpoints.clone();
    let spawned = shell.tasks.spawn(Kind::Audit, move || {
        let home = crate::home::Home::open(&root)?;
        let pile = home.ledger()?.pile()?;
        let g = with_senders(g, &pile.items);
        if !same_basis(&held.fragment, &g) {
            if eps.is_empty() {
                return Err(crate::fault::Fault::known(crate::fault::Known::NoEndpoint, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail021).to_string())));
            }
            return run_audit(&root, &eps, g);
        }
        let v = crate::auditx::ask_from(&pile.items, &held.fragment, held.unanswered.clone(), held.asked, held.single_source)?;
        Ok(Done::Audited {
            broken: v.broken(),
            label: v.label,
            complete: v.complete,
            entries: v.entries,
            report: v.report,
            unanswered: v.unanswered,
            unproven: held.unproven.clone(),
            asked: v.asked,
            single_source: v.single_source,
            fragment: v.fragment,
            recounted: true,
        })
    });
    if matches!(spawned, Spawned::Started) {
        shell.audit_asked = Some(shell.book_mark);
        return true;
    }
    false
}

fn same_basis(fragment: &zikaron::json::Value, g: &crate::auditx::Ground) -> bool {
    use zikaron::json::Value;
    let low = |v: &Value| -> Vec<String> {
        let mut out: Vec<String> = match v {
            Value::Arr(xs) => xs.iter().filter_map(Value::as_str).map(str::to_ascii_lowercase).collect(),
            _ => Vec::new(),
        };
        out.sort();
        out.dedup();
        out
    };
    let mut read: Vec<(u64, u64, Vec<String>, Vec<String>)> = match fragment.member("basis").and_then(|b| b.member("chains")) {
        Some(Value::Arr(cs)) => cs
            .iter()
            .filter_map(|c| match (c.member("chainId"), c.member("fromBlock")) {
                (Some(Value::Int(id)), Some(Value::Int(from))) => Some((*id, *from, low(c.member("registries")?), low(c.member("senders")?))),
                _ => None,
            })
            .collect(),
        _ => return false,
    };
    let senders = low(&Value::Arr(g.senders.iter().map(|s| Value::Str(s.clone())).collect()));
    let regs = |chain: u64, registry: &crate::key::Address| -> Vec<String> {
        let mut out: Vec<String> = zikaron_anchor::revoke::registries(chain, &[registry.0]).iter().map(|r| zikaron::hexfmt::encode(r).to_ascii_lowercase()).collect();
        out.sort();
        out.dedup();
        out
    };
    let mut now: Vec<(u64, u64, Vec<String>, Vec<String>)> = std::iter::once((g.chain, g.from_block, regs(g.chain, &g.registry), senders.clone()))
        .chain(g.earlier.iter().map(|w| (w.chain, w.from_block, regs(w.chain, &w.registry), senders.clone())))
        .collect();
    read.sort();
    now.sort();
    read == now
}

pub fn merge_sent(shell: &mut Shell, chain: u64, tx: &str, block: u64, sent: usize, kept: Option<&zikaron_send::batch::Kept>) -> bool {
    let Some(held) = shell.audit.clone() else { return false };
    if held.nodes != shell.endpoints || held.fragment.member("anchors").is_none() || sent == 0 {
        return false;
    }
    let Some(kept) = kept.filter(|k| held.single_source || !k.single_source).cloned() else { return false };
    let Ok(g) = ground(shell) else { return false };
    let (Some(from), Some(tx)) = (shell.anchor.map(|a| a.0), zikaron::hexfmt::decode(tx).and_then(|b| <[u8; 32]>::try_from(b).ok())) else { return false };
    let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) else { return false };
    let eps = shell.endpoints.clone();
    let spawned = shell.tasks.spawn(Kind::Audit, move || {
        let home = crate::home::Home::open(&root)?;
        let pile = home.ledger()?.pile()?;
        let g = with_senders(g, &pile.items);
        if !same_basis(&held.fragment, &g) {
            return run_audit(&root, &eps, g);
        }
        let Some(fragment) = with_sent(&held.fragment, &kept.anchors, chain, &tx, block, &from, sent) else {
            return run_audit(&root, &eps, g);
        };
        let v = crate::auditx::ask_from(&pile.items, &fragment, held.unanswered.clone(), held.asked, held.single_source)?;
        Ok(Done::Audited {
            broken: v.broken(),
            label: v.label,
            complete: v.complete,
            entries: v.entries,
            report: v.report,
            unanswered: v.unanswered,
            unproven: held.unproven.clone(),
            asked: v.asked,
            single_source: v.single_source,
            fragment: v.fragment,
            recounted: false,
        })
    });
    if matches!(spawned, Spawned::Started) {
        shell.audit_asked = Some(shell.book_mark);
        return true;
    }
    false
}

fn with_sent(fragment: &zikaron::json::Value, read: &[zikaron::json::Value], chain: u64, tx: &[u8; 32], block: u64, from: &[u8; 20], sent: usize) -> Option<zikaron::json::Value> {
    use zikaron::json::Value;
    let (tx_hex, from_hex) = (zikaron::hexfmt::encode(tx), zikaron::hexfmt::encode(from));
    let new: Vec<Value> = read
        .iter()
        .filter(|a| {
            a.member("chainId").and_then(Value::as_int) == Some(chain)
                && a.member("blockNumber").and_then(Value::as_int) == Some(block)
                && a.member("tx").and_then(Value::as_str).is_some_and(|t| t.eq_ignore_ascii_case(&tx_hex))
                && a.member("sender").and_then(Value::as_str).is_some_and(|s| s.eq_ignore_ascii_case(&from_hex))
        })
        .cloned()
        .collect();
    if new.len() != sent {
        return None;
    }
    let Value::Obj(mut top) = fragment.clone() else { return None };
    let sender = zikaron::hexfmt::encode(from).to_ascii_lowercase();
    let mut reached = false;
    for (k, v) in top.iter_mut() {
        match (k.as_str(), v) {
            ("anchors", Value::Arr(xs)) => {
                for a in &new {
                    if !xs.contains(a) {
                        xs.push(a.clone());
                    }
                }
            }
            ("basis", Value::Obj(b)) => {
                let Some((_, Value::Arr(chains))) = b.iter_mut().find(|(k, _)| k == "chains") else { return None };
                let takes = |c: &Value| {
                    matches!(c.member("chainId"), Some(Value::Int(id)) if *id == chain)
                        && matches!(c.member("fromBlock"), Some(Value::Int(f)) if *f <= block)
                        && matches!(c.member("senders"), Some(Value::Arr(ss)) if ss.iter().filter_map(Value::as_str).any(|s| s.eq_ignore_ascii_case(&sender)))
                };
                let from_of = |c: &Value| match c.member("fromBlock") {
                    Some(Value::Int(f)) => *f,
                    _ => 0,
                };
                let Some(w) = chains.iter_mut().filter(|c| takes(c)).max_by_key(|c| from_of(c)) else { return None };
                let Value::Obj(ms) = w else { return None };
                for (k, v) in ms.iter_mut() {
                    if let ("toBlock", Value::Int(to)) = (k.as_str(), v) {
                        *to = (*to).max(block);
                        reached = true;
                    }
                }
            }
            _ => {}
        }
    }
    reached.then_some(Value::Obj(top))
}

pub(super) fn set_basis(
    shell: &mut Shell,
    chain: &str,
    registry: &str,
    from_block: &str,
) -> Result<Basis, crate::fault::Fault> {
    let c: u64 = crate::fault::whole_within_ceiling(chain, crate::lang::Key::Tail024)?;
    let r = Address::parse(registry).ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::AddressShape, registry.trim().to_string())
    })?;
    let f: u64 = crate::fault::whole_within_ceiling(from_block, crate::lang::Key::Tail025)?;
    if crate::deploy::KNOWN.iter().any(|d| d.chain_id == c && d.registry_address() == r) {
        write_basis(shell, c, r, f)?;
        shell.basis_read = None;
        return Ok(Basis::Saved(c));
    }
    let nodes: Vec<String> = shell.endpoints.iter().filter(|e| e.chain == c).map(|e| e.url.for_transport().to_string()).collect();
    if nodes.is_empty() {
        write_basis(shell, c, r, f)?;
        shell.basis_read = Some((c, r, None));
        return Ok(Basis::Saved(c));
    }
    Ok(Basis::Checking(spawn_check(shell, c, r, f, nodes, false)))
}

fn spawn_check(shell: &mut Shell, c: u64, r: Address, f: u64, nodes: Vec<String>, after_nodes: bool) -> Spawned {
    let asked = basis_stamp(shell, c);
    let net = crate::readnets::Net { chain_id: c, registry: r, from_block: f, nodes, name: None };
    shell.tasks.spawn(Kind::Basis, move || {
        let (reading, said) = crate::widex::gate_said(&net);
        Ok(Done::BasisRead { chain: c, registry: r, from_block: f, reading, said, after_nodes, asked })
    })
}

pub(super) fn check_basis_after_nodes(shell: &mut Shell) {
    let (Some(c), Some(r)) = (shell.settings.chain_id, shell.settings.registry) else { return };
    if crate::deploy::KNOWN.iter().any(|d| d.chain_id == c && d.registry_address() == r) {
        return;
    }
    let nodes: Vec<String> = shell.endpoints.iter().filter(|e| e.chain == c).map(|e| e.url.for_transport().to_string()).collect();
    if nodes.is_empty() {
        return;
    }
    let f = shell.settings.from_block;
    if let Spawned::Started = spawn_check(shell, c, r, f, nodes, true) {
        shell.basis_read = Some((c, r, None));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BasisStamp {
    pub cell: u64,
    pub nodes: u64,
}

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

pub(super) enum Basis {
    Saved(u64),
    Checking(Spawned),
}

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

fn write_basis(shell: &mut Shell, c: u64, r: Address, f: u64) -> Result<(), crate::fault::Fault> {
    let before = crate::chainx::nodes_of(&shell.settings);
    shell.commit_settings(|s| {
        s.chain_id = Some(c);
        s.registry = Some(r);
        s.from_block = f;
        s.network = None;
    })?;
    if crate::chainx::nodes_of(&shell.settings) != before {
        super::archive::endpoints_changed(shell);
    }
    if let Err(f) = super::archive::remember_custom(shell) {
        shell.faults.push(f);
    }
    shell.tail_asked = None;
    tail_if_due(shell);
    Ok(())
}
