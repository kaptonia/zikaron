use super::*;

pub(super) fn move_chain(shell: &mut Shell, chain: &str, registry: &str, from_block: &str, nodes: &str) -> Result<(usize, bool), crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    shell.may_write_entries()?;
    let c: u64 = crate::fault::whole_within_ceiling(chain, crate::lang::Key::Tail024)?;
    let r = Address::parse(registry).ok_or_else(|| Fault::known(Known::AddressShape, registry.trim().to_string()))?;
    let f: u64 = crate::fault::whole_within_ceiling(from_block, crate::lang::Key::Tail025)?;
    let (Some(old_chain), Some(old_registry)) = (shell.settings.chain_id, shell.settings.registry) else {
        return Err(Fault::known(Known::NoChainId, String::new()));
    };
    if shell.basis_chains().iter().any(|(x, _, _)| *x == c) {
        return Err(Fault::known(Known::ChainInBasis, c.to_string()));
    }
    if shell.queue.items.iter().any(|q| q.step.in_flight()) {
        return Err(Fault::known(Known::InFlight, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::TaskAnchor).to_string())));
    }
    let Some(report) = shell.audit_report_in_step().cloned() else {
        let _ = super::audit::start_audit(shell);
        return Err(Fault::known(if shell.audit.is_none() { Known::NotAudited } else { Known::AuditBehind }, String::new()));
    };
    let specs: Vec<String> = nodes.split_whitespace().map(|u| if u.contains('=') { u.to_string() } else { format!("{c}={u}") }).collect();
    for s in &specs {
        crate::chainx::typed(s).map_err(|w| Fault::known(Known::SettingsShape, w))?;
    }
    let home = crate::home::Home::bare(shell.home.as_ref().ok_or_else(|| Fault::known(Known::NoHome, String::new()))?.root().to_path_buf());
    let items = home.ledger()?.pile()?.items;
    let (ids, left_out) = moved_ids(&items, &report, &shell.on_chain(), shell.anchor.map(|a| a.hex()).as_deref());
    crate::queue::amend(&home, |_| ())?;
    let earlier = zikaron_seal::ground::Earlier { chain_id: old_chain, registry: old_registry.0, from_block: shell.settings.from_block };
    let mut endpoints = shell.settings.endpoints.clone();
    for s in specs {
        if !endpoints.contains(&s) {
            endpoints.push(s);
        }
    }
    shell.commit_settings(|s| {
        s.earlier.push(earlier);
        s.chain_id = Some(c);
        s.registry = Some(r);
        s.from_block = f;
        s.network = None;
        s.endpoints = endpoints;
    })?;
    super::archive::endpoints_changed(shell);
    let at = now_secs();
    let (n, q) = crate::queue::amend(&home, |q| ids.iter().filter(|id| q.push_on(id, c, at).landed()).count())?;
    shell.queue = q;
    shell.tail_asked = None;
    shell.reread(crate::shell::Reread::Wrote);
    Ok((n, left_out))
}

pub fn moved_ids(items: &[Vec<u8>], report: &zikaron::json::Value, on: &crate::chainx::OnChain, me: Option<&str>) -> (Vec<String>, bool) {
    use zikaron::v2::tokens::Key;
    let mut ledger = crate::auditx::entry_ids(report, Key::Anchored);
    ledger.extend(items.iter().filter_map(|b| zikaron_glue::book::check(b).ok()).map(|e| e.id_hex().to_ascii_lowercase()).filter(|id| on.anywhere(id)));
    ledger.extend(crate::auditx::entry_ids(report, Key::Unanchored));
    if let Some(c) = report.member("continues").and_then(|c| c.member("report")) {
        ledger.extend(crate::auditx::entry_ids(c, Key::Anchored));
        ledger.extend(crate::auditx::entry_ids(c, Key::Unanchored));
    }
    let entries: Vec<zikaron_glue::book::Entry> = items.iter().filter_map(|b| zikaron_glue::book::check(b).ok()).filter(|e| ledger.contains(&e.id_hex().to_ascii_lowercase())).collect();
    let mut lineage: Vec<String> = entries.iter().filter(|e| e.law == zikaron_glue::book::Law::One && e.seq == 0).map(|e| e.author.to_ascii_lowercase()).collect();
    lineage.extend(entries.iter().filter(|e| e.law == zikaron_glue::book::Law::One).filter_map(|e| e.to()).map(str::to_ascii_lowercase));
    let whole = me.is_some_and(|m| lineage.contains(&m.to_ascii_lowercase()));
    let mut keep: Vec<&zikaron_glue::book::Entry> = entries.iter().filter(|e| e.law == zikaron_glue::book::Law::Two || whole || e.closes()).collect();
    keep.sort_by_key(|e| (e.law, e.seq));
    let left_out = !whole && entries.iter().any(|e| e.law == zikaron_glue::book::Law::One && !e.closes());
    (keep.into_iter().map(|e| e.id_hex()).collect(), left_out)
}
