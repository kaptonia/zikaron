use super::*;

/// Import a grant document: take (the payload is decoded by the kit crate), admit (the core's thirteen steps
/// plus two shape gates), store (into the vault, refused when present). A payload with several grants imports
/// them all; if any is refused, none are imported this pass (half a vault is worse than none).
///
/// Admit all first, then store all. Items already in the vault with identical bytes are skipped (a chain's
/// root is usually already in the vault, and a relicense payload brings the root along); items present with
/// different bytes are refused by name before the first is stored. Storing one by one would fail at once on
/// `ALREADY_HELD` when the root was present, the chain's new hop would never land, and the face would only
/// say "already in the vault"; the other way round, the new one would land first and the old would then fail,
/// changing the vault while the face reported failure.
pub(super) fn import_grant(shell: &mut Shell, typed: &str) -> Result<Vec<String>, crate::fault::Fault> {
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    // The grant file form: opened and verified before taking (in `payloadx::take_full`); the chain comes from
    // the grant code inside it.
    let taken = crate::payloadx::take_full(typed)?;
    let all = taken.hops;
    let mut fresh: Vec<&Vec<u8>> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    for b in &all {
        let e = crate::vaultx::admit(b)?;
        let id = e.id_hex();
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
    // A copy of the grant file is kept in the vault (the issuer ledger and terms documents it carries are
    // material at the "vault" level for checks and re-checks). It is kept before storing: if it cannot be
    // kept the whole pass is refused, so the vault never holds a grant "from a grant file whose material was
    // not kept".
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
    // Stage everything aside first and move each into place only when all are staged: if the disk fills up at
    // the second, the first does not remain either (half a vault is worse than none).
    let ids = crate::vaultx::store_all(home, &fresh)?;
    // After storing, the vault must be listable (the disk read runs as a separate background task, not in the
    // frame); a failed listing only records a trouble.
    relist_held(shell);
    Ok(ids)
}

/// Import an existing grant directory: each file whose name matches the store crate's entry names goes
/// through `import_grant` (admit before storing; failures are not stored); those already in the vault with
/// identical bytes are skipped. When none were taken and there were refusals, the last refusal is reported.
///
/// The grant directory is read by the store crate's names (the same naming rule as `verifyx::entries_of`
/// reading a directory): `.DS_Store`, `._*` and other side files dropped by the system are not entries, not
/// refusals, and do not turn a full import into a partial one.
pub(super) fn import_grant_dir(shell: &mut Shell, dir: &str) -> Result<(Vec<String>, Vec<crate::fault::Fault>), crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    let d = dir.trim();
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(d)
        .map_err(|e| crate::fault::classify(&e, d))?
        .filter_map(|x| x.ok().map(|x| x.path()))
        .filter(|p| p.is_file())
        .filter(|p| {
            p.file_name()
                .map(|n| zikaron_store::layout::parse_entry_file(&n.to_string_lossy()).is_some())
                .unwrap_or(false)
        })
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(Fault::known(Known::DirEmpty, d.to_string()));
    }
    // Every file has an outcome: taken, already in the vault (skipped), not taken (with reason). Recording
    // only the last refusal and reporting success when any one was taken would make the person think the
    // whole directory went in.
    let mut ids = Vec::new();
    let mut refused = Vec::new();
    for p in files {
        match import_grant(shell, &p.display().to_string()) {
            Ok(mut got) => ids.append(&mut got),
            Err(f) if f.which() == Some(Known::AlreadyHeld) => {}
            Err(f) => refused.push(f),
        }
    }
    Ok((ids, refused))
}

/// List the vault. Runs on a background thread.
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
    // Empty removes that entry; non-empty is made absolute before saving (`home::kept`).
    let d = if dir.trim().is_empty() { String::new() } else { crate::home::kept(dir)?.display().to_string() };
    shell.commit_settings(|s| {
        s.upstreams.retain(|(x, _)| *x != g);
        if !d.is_empty() {
            s.upstreams.push((g.clone(), d));
        }
    })?;
    Ok(g)
}

/// Name a held grant and its issuer for this machine only. The grant's name is kept by grant id, the
/// issuer's by the issuer's address, so every grant from that issuer shows it. An empty name changes nothing.
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
        let a = held.iter().find(|h| crate::lastread::grant_form(&h.id) == g).map(|h| h.author.to_ascii_lowercase());
        Some(a.ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::ContentShape, g.clone()))?)
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

/// Re-check the whole vault. Runs on a background thread. For each item: upstream bytes read per the
/// bookkeeping, the basis scanned to the chain head along that ledger's lineage, six checks to the kit crate,
/// label to the core, now from the chain.
pub(super) fn review(shell: &mut Shell) -> Result<Spawned, crate::fault::Fault> {
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let picked = crate::vaultx::held(home)?;
    let ground = ground_bare(shell);
    let eps = shell.endpoints.clone();
    // Upstream bytes are resolved level by level as on the check page (`supplyx`: this machine, vault, record
    // bundle, publish address); a level that cannot be read goes onto the card by name.
    let shelf = shelf_of(shell, "");
    let chain = shell.settings.chain_id;
    Ok(shell.tasks.spawn(Kind::Review, move || {
        crate::task::stage_at(Kind::Review, 0);
        let now = chain.and_then(|c| crate::chainx::head_time(&eps, c).ok()).map(|(t, _, _)| t);
        let mut cards = Vec::new();
        crate::task::stage_at(Kind::Review, 1);
        for (i, h) in picked.iter().enumerate() {
            crate::task::count(Kind::Review, i as u64, picked.len() as u64);
            let found = crate::supplyx::find(&shelf, crate::supplyx::Want::Hop(&h.bytes), 0);
            let up: Vec<Vec<u8>> = found.supply.as_ref().map(|s| s.items.clone()).unwrap_or_default();
            // Fragment: scanned only with upstream bytes and a reachable chain; otherwise an empty fragment
            // (those checks honestly answer UNKNOWN).
            let (fragment, anchors) = match (&ground, up.is_empty(), eps.is_empty()) {
                (Ok(g), false, false) => {
                    let who = crate::readerx::who(&h.author)?;
                    match to_head(&eps, g.clone()).and_then(|g| {
                        crate::auditx::scan_once(&eps, &crate::readerx::basis_for(&g, &who, &up))
                    }) {
                        Ok(sc) => (sc.fragment, sc.anchors),
                        Err(_) => (crate::auditx::empty_fragment(), 0),
                    }
                }
                _ => (crate::auditx::empty_fragment(), 0),
            };
            let mut card = crate::vaultx::review(h, &up, &fragment, now, anchors);
            card.from = found.supply.as_ref().map(|s| (s.level, s.place.clone()));
            if card.said.is_empty() && up.is_empty() {
                if let Some((_, f)) = found.misses.first() {
                    card.said = f.evidence();
                }
            }
            cards.push(card);
        }
        Ok(Done::Reviewed { cards, now })
    }))
}
