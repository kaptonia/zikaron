use super::*;

/// Imports a grant document: decode the payload (via the kit crate), admit each entry (the core's checks plus
/// two shape checks), and store it in the vault. A payload with several grants imports them all; if any is
/// refused, none are imported (half a vault is worse than none).
///
/// All entries are admitted before any is stored. Entries already in the vault with identical bytes are
/// skipped (a chain's root is usually there already, and a relicense payload brings it along); entries present
/// with different bytes are refused by name before anything is stored. Storing one by one would stop at
/// `ALREADY_HELD` on the existing root and never store the new hop, or store the new one and then fail,
/// changing the vault while reporting failure.
pub(super) fn import_grant(shell: &mut Shell, typed: &str) -> Result<(Vec<String>, String), crate::fault::Fault> {
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    // A grant file is opened and verified before taking (`payloadx::take_full`); the chain comes from the grant
    // code inside it.
    let taken = crate::payloadx::take_full(typed)?;
    let all = taken.hops;
    let mut fresh: Vec<&Vec<u8>> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    // The grant this import is for is the chain's last hop (codes and grant files carry the chain from the root
    // down); the hops above it are its upstreams, taken along.
    let mut end = String::new();
    for b in &all {
        let e = crate::vaultx::admit(b)?;
        let id = e.id_hex();
        end = id.clone();
        let at = crate::vaultx::path_of(home, &id)?;
        if at.exists() {
            let have = crate::local::read(&at, crate::local::Doc::Held)?.unwrap_or_default();
            if have == **b {
                skipped.push(id);
                continue;
            }
            return Err(crate::fault::Fault::known(crate::fault::Known::Occupied, crate::lang::filln(crate::lang::Key::Tail058, &[&(id).to_string()])));
        }
        fresh.push(b);
    }
    // Keep a copy of the grant file in the vault (the issuer ledger and terms documents it carries are
    // vault-level material for checks and re-checks). It is kept before storing: if it cannot be kept the
    // whole import is refused, so the vault never holds a grant whose grant file material was lost.
    if let Some((p, o)) = taken.file.as_ref() {
        let bytes = std::fs::read(p).map_err(|x| crate::fault::classify(&x, &p.display().to_string()))?;
        crate::grantfilex::keep(home, &bytes, &o.kit_id)?;
    }
    if fresh.is_empty() {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::AlreadyHeld,
            skipped.first().cloned().unwrap_or_default(),
        ));
    }
    // Stage everything aside and move entries into place only once all are staged, so a disk filling up at the
    // second leaves no first behind.
    let ids = crate::vaultx::store_all(home, &fresh)?;
    // Relist the vault after storing (in a separate background task); a failed listing is only reported.
    relist_held(shell);
    Ok((ids, end))
}

/// Imports a directory of grants: every file goes through `import_grant`, the single definition of what a
/// grant is (an entry file, a grant file or a grant code, admitted before storing; each failure named). Files
/// already in the vault with identical bytes are skipped. A folder with no files is refused by name
/// (`GRANT_DIR_EMPTY`).
///
/// System side files (`home::is_side_file`: `.DS_Store`, `._*` and the like) are ignored: not tried, not
/// counted as refusals, and they never turn a full import into a partial one.
pub(super) fn import_grant_dir(shell: &mut Shell, dir: &str) -> Result<(Vec<String>, Vec<crate::fault::Fault>), crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    let d = dir.trim();
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(d)
        .map_err(|e| crate::fault::classify(&e, d))?
        .filter_map(|x| x.ok().map(|x| x.path()))
        .filter(|p| p.is_file())
        .filter(|p| p.file_name().map(|n| !crate::home::is_side_file(&n.to_string_lossy())).unwrap_or(false))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(Fault::known(Known::GrantDirEmpty, d.to_string()));
    }
    // Every file gets an outcome: taken, already in the vault (skipped), or refused with a reason, so a partial
    // import is never reported as the whole directory going in.
    let mut ids = Vec::new();
    let mut refused = Vec::new();
    for p in files {
        match import_grant(shell, &p.display().to_string()) {
            Ok((mut got, _)) => ids.append(&mut got),
            Err(f) if f.which() == Some(Known::AlreadyHeld) => {}
            Err(f) => refused.push(f),
        }
    }
    Ok((ids, refused))
}

/// Lists the vault on a background thread.
pub(super) fn list_held(shell: &mut Shell) -> Result<Spawned, crate::fault::Fault> {
    let root = shell
        .home
        .as_ref()
        .map(|h| h.root().to_path_buf())
        .ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    Ok(shell.tasks.spawn(Kind::Held, move || {
        let home = crate::home::Home::open(&root)?;
        let (held, rejected) = crate::vaultx::held_all(&home)?;
        Ok(Done::Held { held, rejected })
    }))
}

pub(super) fn set_upstream(shell: &mut Shell, grant: &str, dir: &str) -> Result<String, crate::fault::Fault> {
    let g = grant.trim().to_string();
    if !zikaron::hexfmt::is_hex32(&g) {
        return Err(crate::fault::Fault::known(crate::fault::Known::ContentShape, g));
    }
    // Empty removes the entry; otherwise the path is made absolute before saving (`home::kept`).
    let d = if dir.trim().is_empty() { String::new() } else { crate::home::kept(dir)?.display().to_string() };
    shell.commit_settings(|s| {
        s.upstreams.retain(|(x, _)| *x != g);
        if !d.is_empty() {
            s.upstreams.push((g.clone(), d));
        }
    })?;
    Ok(g)
}

/// Names a held grant and its issuer, on this machine only. The grant's name is keyed by grant id and the
/// issuer's by address, so every grant from that issuer shows it. An empty name changes nothing.
pub(super) fn note_held(shell: &mut Shell, grant: &str, note: &str, issuer_note: &str) -> Result<String, crate::fault::Fault> {
    let g = crate::lastread::grant_form(grant);
    if !zikaron::hexfmt::is_hex32(&g) {
        return Err(crate::fault::Fault::known(crate::fault::Known::ContentShape, grant.trim().to_string()));
    }
    let (note, issuer_note) = (note.trim().to_string(), issuer_note.trim().to_string());
    if note.is_empty() && issuer_note.is_empty() {
        return Ok(g);
    }
    let author = if issuer_note.is_empty() {
        None
    } else {
        let home = shell.home.as_ref().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
        let held = crate::vaultx::held(home)?;
        let a = held.iter().find(|h| crate::lastread::grant_form(&h.id) == g).map(|h| crate::lastread::issuer_form(&h.author));
        Some(a.ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::GrantNotHeld, g.clone()))?)
    };
    shell.commit_settings(|s| {
        if !note.is_empty() {
            s.grant_notes.retain(|(x, _)| *x != g);
            s.grant_notes.push((g.clone(), note.clone()));
        }
        if let Some(a) = &author {
            s.issuer_notes.retain(|(x, _)| x != a);
            s.issuer_notes.push((a.clone(), issuer_note.clone()));
        }
    })?;
    Ok(g)
}

/// Re-checks the whole vault on a background thread. For each item, upstream bytes are looked up from the
/// recorded places; then the chain is read across networks as on the check page (`widex`: the main network if
/// configured and every read-only network, each chain separately), with one scan per upstream lineage shared
/// by every grant of that lineage (`vaultx::review_all`). The six checks come from the kit crate, the label
/// from the core, and "now" from the chain. A network that could not be read is named on each card it affects.
pub(super) fn review(shell: &mut Shell) -> Result<Spawned, crate::fault::Fault> {
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let picked = crate::vaultx::held(home)?;
    let ground = ground_bare(shell);
    let eps = shell.endpoints.clone();
    let nets = read_nets_now()?;
    // Upstream bytes are looked up level by level as on the check page (`supplyx`: this machine, vault, record
    // bundle, publish address); a level that cannot be read is named on the card.
    let shelf = shelf_of(shell, "");
    let chain = shell.settings.chain_id;
    Ok(shell.tasks.spawn(Kind::Review, move || {
        crate::task::stage_at(Kind::Review, 0);
        let now = chain.and_then(|c| crate::chainx::head_time(&eps, c).ok()).map(|(t, _, _)| t);
        crate::task::stage_at(Kind::Review, 1);
        let mut items = Vec::with_capacity(picked.len());
        let mut from = Vec::with_capacity(picked.len());
        for (i, h) in picked.iter().enumerate() {
            crate::task::count(Kind::Review, i as u64, picked.len() as u64);
            let found = crate::supplyx::find(&shelf, crate::supplyx::Want::Hop(&h.bytes), 0);
            let up: Vec<Vec<u8>> = found.supply.as_ref().map(|s| s.items.clone()).unwrap_or_default();
            from.push((found.supply.as_ref().map(|s| (s.level, s.place.clone())), found.misses.first().map(|(_, f)| f.evidence())));
            items.push((h.clone(), up));
        }
        // A chain to read: the main network with its nodes, or any read-only network. Otherwise every card is
        // checked against an empty fragment, so its chain checks read as unknown.
        let main = ground.as_ref().ok().filter(|_| !eps.is_empty()).map(|g| (&eps[..], g));
        let mut scan = |senders: &[String]| -> Result<crate::vaultx::LineageRead, String> {
            let w = crate::widex::scan(main, &nets, senders, crate::widex::Ask::First).map_err(|f| f.evidence())?;
            Ok(crate::vaultx::LineageRead { fragment: crate::widex::with_unread_windows(&w.fragment, &w.missed, senders), anchors: w.anchors, missed: w.missed })
        };
        let read: Option<&mut dyn FnMut(&[String]) -> Result<crate::vaultx::LineageRead, String>> = if main.is_some() || !nets.is_empty() { Some(&mut scan) } else { None };
        let mut cards = crate::vaultx::review_all(&items, now, read);
        for (card, ((level, miss), (_, up))) in cards.iter_mut().zip(from.into_iter().zip(items.iter())) {
            card.from = level;
            if card.said.is_empty() && up.is_empty() {
                if let Some(said) = miss {
                    card.said = said;
                }
            }
        }
        Ok(Done::Reviewed { cards, now })
    }))
}
